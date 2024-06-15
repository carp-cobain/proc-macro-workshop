use proc_macro2::{token_stream, Delimiter, Group, Literal, TokenStream, TokenTree};
use syn::{
    parse::{Parse, ParseStream},
    Ident, LitInt, Result, Token,
};

/// This macro provides a syntax for stamping out sequentially indexed copies of an
/// arbitrary chunk of code.
#[proc_macro]
pub fn seq(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse input tokens into a custom syntax tree.
    let ast = syn::parse_macro_input!(input as SeqAst);
    // Perform proc macro expansion and hand the output tokens back to the compiler.
    ast.expand().into()
}

/// Structure of the seq macro abstract syntax tree.
#[derive(Debug)]
struct SeqAst {
    ident: Ident,
    from: LitInt,
    to: LitInt,
    inclusive: bool,
    content: TokenStream,
}

impl Parse for SeqAst {
    // Parse the seq macro abstract syntax tree.
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = Ident::parse(input)?;
        <Token![in]>::parse(input)?;
        let from = LitInt::parse(input)?;
        let inclusive = input.peek(Token![..=]);
        if inclusive {
            <Token![..=]>::parse(input)?;
        } else {
            <Token![..]>::parse(input)?;
        }
        let to = LitInt::parse(input)?;
        let content;
        let _braces = syn::braced!(content in input);
        let content = TokenStream::parse(&content)?;
        Ok(Self {
            ident,
            from,
            to,
            inclusive,
            content,
        })
    }
}

impl SeqAst {
    // Top level macro expansion
    fn expand(&self) -> TokenStream {
        // Check for and expand any sections.
        let (mut expanded, found) = self.expand_sections(self.content.clone());
        if !found {
            // No sections found, check for basic `~N` replacements
            expanded = self.expand_range(self.content.clone());
        }
        expanded
    }

    // Look for and expand any sections defined in `#(...)*`
    fn expand_sections(&self, stream: TokenStream) -> (TokenStream, bool) {
        let mut tokens = TokenStream::new();
        let mut found = false;
        let mut itr = stream.into_iter();
        while let Some(tt) = itr.next() {
            let (ts, fnd) = self.expand_section(tt, &mut itr);
            found |= fnd;
            tokens.extend(ts);
        }
        (tokens, found)
    }

    // Macro expansion helper to check for and expand a section in the token tree.
    fn expand_section(
        &self,
        tt: TokenTree,
        itr: &mut token_stream::IntoIter,
    ) -> (TokenStream, bool) {
        let mut found = false;
        let ts = match tt {
            TokenTree::Group(g) => {
                let (exp, fnd) = self.expand_sections(g.stream());
                let mut new_group = Group::new(g.delimiter(), exp);
                found |= fnd;
                new_group.set_span(g.span());
                TokenTree::Group(new_group).into()
            }
            TokenTree::Punct(ref p) if p.as_char() == '#' => {
                let mut seek = itr.clone();
                match (seek.next(), seek.next()) {
                    (Some(TokenTree::Group(ref g)), Some(TokenTree::Punct(ref p)))
                        if g.delimiter() == Delimiter::Parenthesis && p.as_char() == '*' =>
                    {
                        // Found `#(...)*` so expand group for range and jump ahead in stream
                        *itr = seek;
                        found = true;
                        self.expand_range(g.stream())
                    }
                    _ => TokenTree::Punct(p.clone()).into(),
                }
            }
            _ => tt.into(),
        };
        (ts, found)
    }

    // Expand a given token stream for the sequence range.
    fn expand_range(&self, stream: TokenStream) -> TokenStream {
        match self.range() {
            Err(err) => err.into_compile_error(),
            Ok(range) => range
                .map(|i| self.expand_index(stream.clone(), i))
                .collect(),
        }
    }

    // Calculate the range of integers from the ast.
    fn range(&self) -> Result<impl Iterator<Item = u64>> {
        let from = self.from.base10_parse::<u64>()?;
        let to = self.to.base10_parse::<u64>()?;
        if from > to {
            return Err(syn::Error::new(self.from.span(), "invalid range"));
        }
        let range = if self.inclusive {
            from..(to + 1)
        } else {
            from..to
        };
        Ok(range)
    }

    // Macro expansion of a stream (note: might be sub-stream) for seq index.
    fn expand_index(&self, stream: TokenStream, i: u64) -> TokenStream {
        let mut tokens = TokenStream::new();
        let mut itr = stream.into_iter();
        while let Some(tt) = itr.next() {
            tokens.extend(self.expand_tree(tt, &mut itr, i));
        }
        tokens
    }

    // Macro expansion helper for a seq index at a position in the token tree.
    fn expand_tree(&self, tt: TokenTree, itr: &mut token_stream::IntoIter, i: u64) -> TokenStream {
        match tt {
            TokenTree::Group(g) => {
                let exp = self.expand_index(g.stream(), i);
                let mut new_group = Group::new(g.delimiter(), exp);
                new_group.set_span(g.span());
                TokenTree::Group(new_group)
            }
            TokenTree::Ident(ref ident) if ident == &self.ident => {
                let mut lit = Literal::u64_unsuffixed(i);
                lit.set_span(ident.span());
                TokenTree::Literal(lit)
            }
            TokenTree::Ident(mut ident) => {
                let mut seek = itr.clone();
                match (seek.next(), seek.next()) {
                    (Some(TokenTree::Punct(ref p)), Some(TokenTree::Ident(ref id)))
                        if p.as_char() == '~' && id == &self.ident =>
                    {
                        // Found `~N` so rewrite identifier in place and jump ahead in stream
                        ident = proc_macro2::Ident::new(&format!("{}{}", ident, i), ident.span());
                        *itr = seek;
                    }
                    _ => {}
                }
                TokenTree::Ident(ident)
            }
            _ => tt,
        }
        .into()
    }
}
