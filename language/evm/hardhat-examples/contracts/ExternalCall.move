#[contract]
module Evm::ExternalCall {
    #[external(sig=b"forty_two() returns (uint64)")]
    public native fun external_call_forty_two(contract: address): u64;

    #[external(sig=b"revertWithMessage()")]
    public native fun external_call_revertWithMessage(contract: address);

    #[callable(sig=b"call_forty_two(address) returns (uint64)"), view]
    public fun call_forty_two(contract: address): u64 {
        external_call_forty_two(contract)
    }

    #[callable(sig=b"call_revertWithMessage(address)"), pure]
    public fun call_revertWithMessage(contract: address) {
        external_call_revertWithMessage(contract);
    }

    #[callable(sig=b"try_call_forty_two(address) returns (uint64)"), view]
    public fun try_call_forty_two(contract: address): u64 {
        // TODO: try-call-catch. See `ExternalCall.sol`.
        external_call_forty_two(contract)
    }

    #[callable(sig=b"try_call_revertWithMessage(address)"), pure]
    public fun try_call_revertWithMessage(contract: address) {
        // TODO: try-call-catch. See `ExternalCall.sol`.
        external_call_revertWithMessage(contract);
    }
}
