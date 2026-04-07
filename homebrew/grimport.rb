cask "grimport" do
  version "0.5.1"
  sha256 "c4c728850568e1c77e4e3701cd796e482e752bb9b2f7ce1eb65580831ab9ed5b"

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
