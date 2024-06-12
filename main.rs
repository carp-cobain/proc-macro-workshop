// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run
//
use derive_builder::Builder;

#[derive(Builder)]
pub struct MyProfile {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    #[builder(each = "tag")]
    pub tags: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let my_profile = MyProfile::builder()
        .name("David".into())
        .email("david@email.com".into())
        .phone("555-1234".into())
        .tag("hockey".into())
        .tag("fly_fishing".into())
        .tag("trail_running".into())
        .build()?;

    println!(
        "name = {}, email = {}, phone = {}, tags = {:?}",
        my_profile.name,
        my_profile.email,
        my_profile.phone.unwrap(),
        my_profile.tags
    );

    Ok(())
}
