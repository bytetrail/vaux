![Build](https://github.com/bytetrail/vaux/actions/workflows/rust-build.yaml/badge.svg)
![Test](https://github.com/bytetrail/vaux/actions/workflows/rust-test.yaml/badge.svg)
![Clippy](https://github.com/bytetrail/vaux/actions/workflows/rust-clippy.yaml/badge.svg)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

![Logo](images/vaux-logo.svg) 


Vaux, pronounced vôks, is an MQTT v5.0 broker built using Rust. 

# Roadmap
Last Update: May 4, 2022

### Basic Session Management
Complete implementation of connect and acknowlegement packets with session creation.
This will include end-to-end testing for client <-> server session establishment 
and disconnect scenarios.

### Command Line Interface
Basic command line interface for broker options.

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
