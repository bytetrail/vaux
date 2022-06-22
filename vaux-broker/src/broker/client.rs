#[derive(Debug, Clone)]
pub struct Client {
    id: String,
    banned: bool,
}

impl Client {
    pub fn new(id: String) -> Self {
        Client { id, banned: false }
    }

    pub fn banned(&self) -> bool {
        self.banned
    }

    pub fn set_banned(&mut self, banned: bool) {
        self.banned = banned;
    }
}
