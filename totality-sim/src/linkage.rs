pub trait Simulated: 'static {
    fn step(time: std::time::Duration, source: &Self, target: &mut Self);
}
pub struct DataLinkageGuard<'link, T: Simulated, DL: DataLinkage<T>> {
    src: &'link DL,
    phantom: std::marker::PhantomData<T>,
}
impl<'link, T: Simulated, DL: DataLinkage<T>> DataLinkageGuard<'link, T, DL> {
    pub fn new(src: &'link DL) -> DataLinkageGuard<'link, T, DL> {
        DataLinkageGuard {
            src,
            phantom: std::marker::PhantomData,
        }
    }
    pub fn source(&self) -> &'link T {
        self.src
            .source()
            .expect("Created with a thing that was not unlocked.")
    }
    pub fn target(&self) -> &'link mut T {
        self.src
            .target()
            .expect("Created with a thing that was not unlocked.")
    }
}
impl<'link, T: Simulated, DL: DataLinkage<T>> Drop for DataLinkageGuard<'link, T, DL> {
    fn drop(&mut self) {
        self.src.cleanup();
    }
}
pub trait DataLinkage<T: Simulated>: 'static + Send + Sized {
    fn advance(&self) -> Option<DataLinkageGuard<T, Self>>;
    fn source(&self) -> Option<&T>;
    fn target(&self) -> Option<&mut T>;
    fn cleanup(&self) {}
}
