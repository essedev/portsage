cask "grimport" do
  version "0.1.0"
  sha256 "ef72bcabc2953c1f7ea25070e7ca8287bf36315537e2912b0c35846ad15a4183"

  url "https://github.com/essedev/grimport/releases/download/v#{version}/Grimport_#{version}_aarch64.dmg"
  name "Grimport"
  desc "Port allocation manager for macOS - your port grimoire"
  homepage "https://github.com/essedev/grimport"

  app "Grimport.app"
end
