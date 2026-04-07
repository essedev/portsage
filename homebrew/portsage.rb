cask "portsage" do
  version "0.7.3"
  sha256 "fd25a42c75606604e24caf25f320eac71b9a45378ef9ef84b2a9caabca5690f7"

  url "https://github.com/essedev/portsage/releases/download/v#{version}/Portsage_#{version}_aarch64.dmg"
  name "Portsage"
  desc "Port allocation manager for macOS - your port sage"
  homepage "https://github.com/essedev/portsage"

  app "Portsage.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Portsage.app"]
  end
end
