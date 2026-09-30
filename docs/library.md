# Rust library usage

`denki` can also be used as a library from another Rust project:

```toml
[dependencies]
denki = { git = "https://github.com/kahwee/denki" }
```

```rust
use denki::{klap, ops};

let json = ops::sysinfo("192.168.1.42").await?;
let mut session = klap::handshake("192.168.1.50", "user@example.com", "pass").await?;
let info = ops::tapo_device_info(&mut session).await?;
ops::tapo_on(&mut session).await?;
```


See [`src/lib.rs`](../src/lib.rs) for public modules. The snippet belongs in an
async function returning `anyhow::Result<()>`; it contacts real devices.
