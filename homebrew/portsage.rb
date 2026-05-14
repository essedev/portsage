cask "portsage" do
  version "0.12.2"
  sha256 "804a2fb378b40b98acecf51b7367a14f178334393d85b3ad3060cccdcaf29ac9"

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
