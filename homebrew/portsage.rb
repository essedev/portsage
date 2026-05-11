cask "portsage" do
  version "0.8.2"
  sha256 "525980bb2e7259154c6d6b2f20e6519dff0bf424a1d9b672c685de9ad99342be"

  url "https://github.com/essedev/portsage/releases/download/v#{version}/Portsage_#{version}_aarch64.dmg"
  name "Portsage"
  desc "Port allocation manager for macOS - ports under control"
  homepage "https://github.com/essedev/portsage"

  app "Portsage.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Portsage.app"]
  end
end
