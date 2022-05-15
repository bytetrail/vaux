![Build](https://github.com/bytetrail/vaux/actions/workflows/rust-build.yaml/badge.svg)
![Test](https://github.com/bytetrail/vaux/actions/workflows/rust-test.yaml/badge.svg)
![Clippy](https://github.com/bytetrail/vaux/actions/workflows/rust-clippy.yaml/badge.svg)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

![Logo](images/vaux-logo.svg) 


Vaux, pronounced vôks, is an MQTT v5.0 broker built using Rust. Vaux is designed
to be a secure, reliable, and performant MQTT broker able to run on a range of 
computing platforms from a Raspberry PI to a server class compute environment in
public cloud infrastructure.


## Usage
```

USAGE:
vaux-broker [OPTIONS]

OPTIONS:
-a, --max-active-sessions <MAX_ACTIVE_SESSIONS>    
-h, --listen-addr <LISTEN_ADDR>                    Listen address (default is "127.0.0.1")s
--help                                         Print help information
-p, --port <PORT>                                  
-s, --max-sessions <MAX_SESSIONS>                  Maximum number of sessions active/in-use
-V, --version                                      Print version information

```

# Roadmap
Last Update: May 15, 2022

### Basic Session Management
Complete implementation of connect and acknowledgement packets with session creation.
This will include end-to-end testing for client <-> server session establishment 
and disconnect scenarios.

### Session Persistence
Serialization and deserialization of sessions to persistent store. Startup deserialization
and evaluation of sessions.

### Publish
Add publication support and basic message management.

### Subscription Management
Add subscribe packet support and basic subscription management

### TLS
Add TLS support. Update the command line and configuration to support running 
with TLS. TLS will be the default startup mode.

### Basic Authentication
Simple username password authentication. 

### Extend Session Management
Support advanced session management scenarios.
