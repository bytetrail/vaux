## auto-ack
The client manages all QoS-1 and QoS-2 control packets when auto-ack is enabled. The 
QoS-1 and 2 control packets are still sent to the client application so that the application 
has them available for any necessary processing, however, the client will automatically
send the appropriate PUBACK, PUBREC, PUBREL, and PUBCOMP control packets for both
incoming and outgoing PUBLISH control packets.

QoS-1  and QoS-2 control packets sent to the MQTT client by the client application are
ignored by the MQTT client when auto-ack is enabled.

### QoS-2 Flow
The flow of QoS-2 control packets is shown in the diagrams below. The MQTT client manages
the flow of QoS-2 control packets between the client and the broker when the application
client send or receives a PUBLISH control packet with QoS-2. 

#### Client Initiated PUBLISH

The MQTT client will send the PUBLISH control packet to the broker and wait for the PUBREC
control packet. The MQTT client will then send the PUBREL control packet to the broker once
the PUBREC has been received and wait for the PUBCOMP control packet.

 ![QOS-2](/images/qos2flow.svg) 

 #### Broker Initiated PUBLISH
 

The MQTT client and session state will manage unacknowledged QoS-2 control packets
and resend the control packets as necessary on a session reconnect with the clean start flag
set to `false`.