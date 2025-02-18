![Build](https://github.com/bytetrail/vaux/actions/workflows/rust-build.yaml/badge.svg)
![Test](https://github.com/bytetrail/vaux/actions/workflows/rust-test.yaml/badge.svg)
![Clippy](https://github.com/bytetrail/vaux/actions/workflows/rust-clippy.yaml/badge.svg)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

![Logo](images/vaux-logo-3.svg) 

Vaux, pronounced vôks, is an MQTT v5.0 broker built using Rust. Vaux is designed
to be a secure, reliable, and performant MQTT broker able to run on a range of 
computing platforms from a Raspberry PI to a server class compute environment in
public cloud infrastructure.

MQTT v3 is on the roadmap; however, is not initially supported.

## Performance and Optimization
Initial versions of the modules and client and server are being built using safe 
Rust without specific runtime optimizations in place. Future performance testing 
on both resource constrained and server class platforms will drive optimization 
efforts.

# Workspace
Vaux workspace consists of 4 projects:

| Project     | Description                                               |
|-------------|-----------------------------------------------------------|
| vaux-broker | A broker library and broker application                   |
| vaux-mqtt   | The MQTT codec library.                                   |
| vaux-client | MQTT client library using vaux-mqtt for embedded devices. |
| vaux-test   | A client test driver for end-to-end integration tests.    |

## vaux-mqtt
This library supports basic MQTT v5.0 control packet encoding and decoding. 
The library is developed with full encoding and decoding support for all MQTT
v5.0 control packets.

Future versions of the library may include default features for client and 
server encoding and decoding support. A library optimized for only 
the encoding or decoding necessary in a client or server implementation will be 
supported. The library would be compiled without CONNACK encoding support, for
example, when a client library is required.

### _Future_
This library currently makes use of many of the Rust standard
library features ```Vec```, ``` HashSet```, ```String```, ```format!``` macro, etc. making it
unsuited for a resource constrained embedded device. Future development will include
a version of the codec that may be used with in an MQTT client library that supports
embedded devices. See _vaux-embedded_

## vaux-client
MQTT v5 client library using the vaux-mqtt codec. The vaux client provides a wrapper around 
the basic MQTT v5.0 protocol that supports clients that need to operate in a continuous 
read/write mode. Currently the vaux MQTT client provides this capability with a separate 
thread for protocol management with channels supporting inbound and outbound traffic 
management.


## vaux-broker
An in-progress effort on a complete implementation of an MQTT v5 broker. See roadmap below.

### Usage
```

USAGE:
vaux-broker [OPTIONS]

OPTIONS:
-a, --max-active-sessions <MAX_ACTIVE_SESSIONS>    
-h,  --help                                        Print help information
-l,  --listen-addr <LISTEN_ADDR>                   Listen address (default is "127.0.0.1")
-p, --port <PORT>                                  
-s, --max-sessions <MAX_SESSIONS>                  Maximum number of sessions active/in-use
-V, --version                                      Print version information

```

# vaux-broker Roadmap
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

### MQTT v3
Add support for MQTT v3 protocol in MQTT codec, client, and broker. 
