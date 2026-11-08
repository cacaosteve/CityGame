import SwiftUI

/// Branded iPadOS host shell. The native Rust/SDL/wgpu game links in once the
/// static library target is wired (see packaging/ios/README.md).
struct ContentView: View {
    var body: some View {
        ZStack {
            Color(red: 0.10, green: 0.12, blue: 0.18)
                .ignoresSafeArea()
            VStack(spacing: 20) {
                Image("BrandLogo")
                    .resizable()
                    .frame(width: 128, height: 128)
                    .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
                Text("CityGame")
                    .font(.system(size: 44, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
                Text("iPadOS host ready — same icon as macOS / Windows / Linux / Android.\nNative sim builds via SDL3 + wgpu are next.")
                    .font(.body)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.white.opacity(0.85))
                    .padding(.horizontal, 32)
            }
        }
    }
}
