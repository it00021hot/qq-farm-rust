pub(crate) struct IllustratedState {
    pub(crate) book: String,
}

impl Default for IllustratedState {
    fn default() -> Self {
        Self { book: "crop".into() }
    }
}
