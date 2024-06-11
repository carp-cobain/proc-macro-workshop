// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run
use derive_debug::CustomDebug;

#[derive(CustomDebug)]
pub struct MyProfile<T> {
    pub name: T,
    pub email: &'static str,
    #[debug = "0x{:02x}"]
    team_id: u8,
}

fn main() {
    let my_profile = MyProfile {
        name: "David",
        email: "david@email.com",
        team_id: 0b10101111,
    };
    println!("my_profile = {:?}", my_profile);
}
