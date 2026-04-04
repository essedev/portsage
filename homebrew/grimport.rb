cask "grimport" do
  version "0.4.0"
  sha256 "e4716a7cc30e0e103a5e939b3b835f8b18395dc8830e9000531174b2c667c631"

  url "https://github.com/essedev/grimport/releases/download/v#{version}/Grimport_#{version}_aarch64.dmg"
  name "Grimport"
  desc "Port allocation manager for macOS - your port grimoire"
  homepage "https://github.com/essedev/grimport"

  app "Grimport.app"
end
