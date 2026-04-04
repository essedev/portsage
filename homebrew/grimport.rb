cask "grimport" do
  version "0.2.0"
  sha256 "568292d24cfee80bfc2aba918b72bdb7e2eacf68e73fc2e13b9e6a839730dc32"

  url "https://github.com/essedev/grimport/releases/download/v#{version}/Grimport_#{version}_aarch64.dmg"
  name "Grimport"
  desc "Port allocation manager for macOS - your port grimoire"
  homepage "https://github.com/essedev/grimport"

  app "Grimport.app"
end
