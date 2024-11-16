<div align="center">
<img src="./.github/scope-round-200.png" />
<h1>Scope</h1>
<h2>discord client for power users</h2>
<a href="https://www.scopeclient.com/">scopeclient.com</a>
</div>

##### Scope is in its earliest stages of development. This readme will be fleshed out as the project progresses.

## Building

### Prerequisites

- [Rust & Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Steps

1. Clone the repository
2. Run `cargo build --release`
3. The binary will be in `./target/release/scope`

### Environment
The binary presently requires the following environment variables to be set or in a `.env` file in the current working directory:
- `DISCORD_TOKEN` - your discord token
- `DEMO_CHANNEL_ID` - the channel ID to listen for messages on

## Developing

### Prerequisites

- [Rust & Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Steps

1. Clone the repository
2. Run `cargo run`

### Environment
The binary presently requires the following environment variables to be set or in a `.env` file in the current working directory:
- `DISCORD_TOKEN` - your discord token
- `DEMO_CHANNEL_ID` - the channel ID to listen for messages on