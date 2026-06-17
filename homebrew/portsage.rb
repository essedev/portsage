cask "portsage" do
  version "0.13.0"
  sha256 "acf62f85ce186ca9e9f1f2debbcd079ece5fbd54bfce58259536f7153ef34729"

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
