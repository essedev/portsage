cask "portsage" do
  version "0.12.1"
  sha256 "775a26a0e0e2e1fd25725ff4fc7130f9a9d6c957ccf8647b3ca486478b686d04"

  url "https://github.com/essedev/portsage/releases/download/v#{version}/Portsage_#{version}_aarch64.dmg"
  name "Portsage"
  desc "Port allocation manager for macOS - ports under control"
  homepage "https://github.com/essedev/portsage"

  app "Portsage.app"
  binary "#{appdir}/Portsage.app/Contents/MacOS/portsage-cli", target: "portsage"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Portsage.app"]
  end
end
