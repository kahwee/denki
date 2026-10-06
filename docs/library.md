# Rust library usage

Requires Rust 1.99 or newer. Add these dependencies to your project:

```toml
[dependencies]
denki = { git = "https://github.com/kahwee/denki" }
anyhow = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Kasa device information

```rust,no_run
use denki::ops;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let info = ops::sysinfo("192.0.2.42").await?;
    println!("{info}");
    Ok(())
}
```

## Tapo device information

Create a KLAP session with your Tapo credentials and reuse it for subsequent calls:

```rust,no_run
use denki::{klap, ops};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let user = std::env::var("TAPO_USER")?;
    let password = std::env::var("TAPO_PASS")?;
    let mut session = klap::handshake("192.0.2.50", &user, &password).await?;
    let info = ops::tapo_device_info(&mut session).await?;
    println!("{info}");
    Ok(())
}
```

Replace these documentation-only addresses with your device's local IP before
running. The examples read information; they do not change power state. CI compiles
these exact examples as doctests without contacting devices.

The low-level `ops` functions take addresses or sessions directly. They do not
resolve CLI aliases or enforce the saved-identity checks used by CLI commands.

## P125 through TPAP/KLAP auto negotiation

The adapter reuses credentials saved by `denki login` or `TAPO_USER`/`TAPO_PASS`.
It checks any identity already bound to the address in the alias registry.

```rust,no_run
# async fn example() -> anyhow::Result<()> {
let info = denki::tapo_client::info("192.0.2.50").await?;
println!("{}: {}", info.model, info.device_on);
# Ok(())
# }
```

`set_power(ip, Some(true))` turns a supported P125 on, `Some(false)` turns it off,
and `None` toggles its observed state. Writes check capabilities and verify the
result with a readback; they are not retried. The client negotiates automatically,
and currently does not expose the selected wire protocol.
