pub enum E<'a> {
    Hover, Click, Dropped(&'a str),
}
