# 8033 scouting

This is a scouting website to track the performance of robots in the 2022 FRC game, Rapid React.
It uses the rust crates actix for serving forms and data and diesel for accessing the SQLite database.
All HTML is embedded into the binary and the database is automatically generated, so no files other than the binary are necessary to start the server, but keep in mind that it will create a database in the working directory and it likely isn't possible to directly copy a local copy of the binary to a server since the server may have different library versions or hardware.

You can run the server directly with cargo:

```sh
cargo run
# Started http server: 127.0.0.1:8080
```

Or build an optimized release binary for real use:

```sh
cargo build --release
target/release/scouting_8033
# Started http server: 127.0.0.1:8080
```
