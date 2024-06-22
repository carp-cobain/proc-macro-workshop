// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run
use sorted::sorted;

#[sorted]
pub enum Token {
    BTC,
    DOGE,
    ETH,
    SOL,
    USDC,
    XRP,
}

#[sorted::check]
fn show(token: Token) {
    #[sorted]
    match token {
        Token::BTC => println!("bitcoin"),
        Token::ETH => println!("ethereum"),
        Token::SOL => println!("solana"),
        _ => println!("shitcoin"),
    }
}

fn main() {
    show(Token::SOL);
}
