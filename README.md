# rpccaps
Capability based RPC library:
- RPC schema is defined directly in the code;
- Capability based object referencing and access;
- Multiplexing over multiple different RPC services;

Under development and not usage ready.


## Example service

```rust
use rpccaps::rpc::Service;
use rpccaps::rpc::transport::MPSCTransport;
use rpccaps_derive::*;

pub struct SimpleService {
    a: u32,
}

impl SimpleService {
    fn new() -> Self {
        Self { a: 0 }
    }
}

#[service]
impl SimpleService {
    fn clear(&mut self) {
        self.a = 0;
    }

    fn add(&mut self, a: u32) -> u32 {
        self.a += a;
        self.a
    }

    async fn sub(&mut self, a: u32) -> u32 {
        self.a -= a;
        self.a
    }

    async fn get(&mut self) -> u32 {
        self.a
    }
}
```

