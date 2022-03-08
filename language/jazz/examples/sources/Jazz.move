module Jazz::Jazz {

    /// A type which represents a continuation.
    native struct Cont<T> has drop;

    /// Yield execution to the given continuation.
    public native fun yield<T>(cont: Cont<T>, result: T);
}
