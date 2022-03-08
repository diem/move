/// Actor support functions.
module Async::Actor {
    /// Returns address of executing actor.
    public native fun self(): address;

    /// Returns the epoch time. This time does not increase during handling of a message. On blockchains, this
    /// is the block timestamp.
    public native fun epoch_time(): u64;

    /// A type to represent a 'unit' (type with a single element).
    native struct Unit has drop, copy, store;
    public native fun unit(): Unit;
}
