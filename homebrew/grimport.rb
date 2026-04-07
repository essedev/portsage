cask "grimport" do
  version "0.5.3"
  sha256 "97e1412627808055099c80e700856974c80556c98218fe1b561cc839e2b752d0"

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
