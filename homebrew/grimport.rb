cask "grimport" do
  version "0.5.0"
  sha256 "0427331cb16f7b217b464e5bd8394573dcbe64f4720d546d53f3c208978322b6"

  url "https://github.com/essedev/grimport/releases/download/v#{version}/Grimport_#{version}_aarch64.dmg"
  name "Grimport"
  desc "Port allocation manager for macOS - your port grimoire"
  homepage "https://github.com/essedev/grimport"

  app "Grimport.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Grimport.app"]
  end
end
