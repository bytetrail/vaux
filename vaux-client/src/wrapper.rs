use std::sync::mpsc::{Receiver, Sender};

use vaux_mqtt::{
    property::{PacketProperties, Property, PropertyBundle},
    publish::Publish,
    Packet, QoSLevel,
};

use crate::{ErrorKind, MqttError, Result};

pub struct ClientWrapper {
    sender: Option<Sender<Packet>>,
    receiver: Option<Receiver<Packet>>,
}

impl ClientWrapper {
    pub fn new() -> Self {
        ClientWrapper {
            sender: None,
            receiver: None,
        }
    }

    pub fn send_binary(
        &mut self,
        topic: &str,
        data: &[u8],
        props: Option<&PropertyBundle>,
    ) -> Result<()> {
        self.publish(topic, data, QoSLevel::AtMostOnce, props)
    }

    pub fn send_utf8(
        &mut self,
        topic: &str,
        message: &str,
        props: Option<&PropertyBundle>,
    ) -> Result<()> {
        self.publish(topic, message.as_bytes(), QoSLevel::AtMostOnce, props)
    }

    /// Basic send of a UTF8 encoded payload. The message is sent with QoS
    /// Level 0 and a payload format indicated a UTF8. No additional properties
    /// are set. A disconnect message can occur due to errors. This method uses
    /// a simple blocking read with a timeout for test purposes.
    pub fn publish(
        &mut self,
        topic: &str,
        data: &[u8],
        qos: QoSLevel,
        props: Option<&PropertyBundle>,
    ) -> Result<()> {
        let mut publish = Publish::default();
        publish
            .properties_mut()
            .set_property(Property::PayloadFormat(
                vaux_mqtt::property::PayloadFormat::Utf8,
            ));
        publish.topic_name = Some(topic.to_string());
        publish.set_payload(Vec::from(data));
        publish.set_qos(QoSLevel::AtLeastOnce);
        publish.packet_id = Some(101);
        if let Some(props) = props {
            publish.set_properties(props.clone());
        }
        if let Some(sender) = &self.sender {
            if sender.send(Packet::Publish(publish)).is_err() {
                return Err(MqttError::new(
                    "unable to send packet to broker",
                    ErrorKind::Transport,
                ));
            }
        } else {
            return Err(MqttError::new(
                "no connection to broker",
                ErrorKind::Transport,
            ));
        }
        Ok(())
    }
}
