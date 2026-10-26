import Foundation
import NetworkExtension

enum Mode { case onlySelected, allExceptSelected }
enum Decision { case protect, direct, block }
enum PolicyError: Error { case invalidIdentity }
struct Policy {
    let mode: Mode
    let signingIDs: Set<String>
    init(mode: Mode, signingIDs: [String]) throws {
        guard signingIDs.allSatisfy({ !$0.isEmpty && $0 == $0.trimmingCharacters(in: .whitespacesAndNewlines) }) else { throw PolicyError.invalidIdentity }
        self.mode = mode
        self.signingIDs = Set(signingIDs)
    }
    // Signing IDs alone are NOT authenticated authorization. Production must validate
    // sourceAppAuditToken against code signing requirement/team and helper ownership.
    func decision(signingID: String?) -> Decision {
        guard let id = signingID, !id.isEmpty else { return .block }
        let selected = signingIDs.contains(id)
        return (mode == .onlySelected ? selected : !selected) ? .protect : .direct
    }
}

func makeCandidateSettings() -> NETransparentProxyNetworkSettings {
    let settings = NETransparentProxyNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
    // nil remote/local criteria: candidate broad TCP/UDP rules, no wildcard:port-0 pair.
    // OS acceptance remains a runtime gate, including DNS and loopback behavior.
    settings.includedNetworkRules = [
        NENetworkRule(remoteNetwork: nil, remotePrefix: 0, localNetwork: nil, localPrefix: 0,
                      protocol: .TCP, direction: .outbound),
        NENetworkRule(remoteNetwork: nil, remotePrefix: 0, localNetwork: nil, localPrefix: 0,
                      protocol: .UDP, direction: .outbound)
    ]
    settings.excludedNetworkRules = []
    return settings
}

func makePacketConfiguration() -> NETunnelProviderProtocol {
    let configuration = NETunnelProviderProtocol()
    configuration.providerBundleIdentifier = "org.example.tavra.packet"
    configuration.serverAddress = "compile-only.invalid"
    configuration.providerConfiguration = ["probeOnly": true]
    return configuration
}

final class PacketTunnelProbe: NEPacketTunnelProvider {
    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        // No packet settings, transport, route changes, or fake connected state.
        completionHandler(NSError(domain: "TavraCompileOnly", code: 1))
    }
    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        completionHandler()
    }
}

final class TransparentProxyProbe: NETransparentProxyProvider {
    // Not loaded from an untrusted UI; no production policy IPC exists yet.
    private let policy = try! Policy(mode: .onlySelected, signingIDs: [])
    override func startProxy(options: [String: Any]? = nil, completionHandler: @escaping (Error?) -> Void) {
        // Deliberately never applies candidate settings; this provider cannot start.
        completionHandler(NSError(domain: "TavraCompileOnly", code: 2))
    }
    override func handleNewFlow(_ flow: NEAppProxyFlow) -> Bool {
        switch policy.decision(signingID: flow.metaData.sourceAppSigningIdentifier) {
        case .direct:
            return false // Transparent proxy: false means DIRECT, never use for protected flow.
        case .protect, .block:
            let error = NSError(domain: "TavraTransportUnimplemented", code: 3)
            flow.closeReadWithError(error)
            flow.closeWriteWithError(error)
            return true // Claim and close. No usable forwarding transport exists in this spike.
        }
    }
}
