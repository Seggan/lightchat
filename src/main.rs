use std::fs::read_to_string;

use crate::se::User;

mod se;

#[tokio::main]
async fn main() {
    let mut client = User::new();
    client.login(
        "seggan21@gmail.com",
        Some(read_to_string("password.txt").unwrap().as_str()),
        "codegolf.stackexchange.com"
    )
        .await
        .unwrap();
    let room = client.join_room(1).unwrap();
    room.send_message("Hello, world?").await.unwrap();
}
