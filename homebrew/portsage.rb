cask "portsage" do
  version "0.5.3"
  sha256 "97e1412627808055099c80e700856974c80556c98218fe1b561cc839e2b752d0"

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
