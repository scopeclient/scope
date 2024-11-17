<div align="center">

<img width="128px" src="./.github/scope-round-200.png" />
<h1><b>Scope</b></h1>

The Discord client for power users.
<br>
<a href="https://www.scopeclient.com/"><strong>scopeclient.com »</strong></a>

<table>
  <tbody>
    <tr>
      <td>No Release Downloads Yet</td>
    </tr>
  </tbody>
</table>

<sup>Nightly releases can be found <a href="https://github.com/scopeclient/scope/actions/workflows/build.yml">here</a>. </sup>

</div>

###### Scope is in its earliest stages of development. This README will be fleshed out as the project progresses.

## Building the Project

### Prerequisites

- [Rust & Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Steps

1. Clone the repository
2. Run `cargo build --release`
3. The binary will be in `./target/release/scope`

## Development Setup

### Prerequisites

- [Rust & Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

### Steps

1. Clone the repository
2. Run `cargo run`
   - It's recommended to use `cargo watch -- cargo run` from [cargo-watch](https://github.com/watchexec/cargo-watch), but it's optional

## Environment Variables

The binary requires the following environment variables to be set in the current working directory or in a `.env` file:

- `DISCORD_TOKEN` - Your Discord token
- `DEMO_CHANNEL_ID` - The channel ID to listen for messages on
