# Harry Potter 2: Chamber of Secrets
#
# The Homebrew cask for the modernized runtime. It installs the application
# only; the copyrighted game data is not distributed here and is imported by
# the user from a retail copy they own.
#
# The formula lives in this repository, which doubles as its own tap:
#   brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
#   brew install --cask cefege/hp2/harry-potter-2

cask "harry-potter-2" do
  version "0.1.0"

  # Stamped by Build/stamp_cask.py during a tagged release, so the checksum
  # always matches the artifact that actually shipped.
  sha256 :no_check

  url "https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases/download/v#{version}/HarryPotter2-#{version}-macos-arm64.dmg",
      verified: "github.com/cefege/harry-potter-2-chamber-of-secrets-pc/"
  name "Harry Potter 2"
  desc "Modernized runtime for Harry Potter and the Chamber of Secrets (2002)"
  homepage "https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc"

  app "HarryPotter2.app"
  depends_on macos: :sequoia

  postflight do
    # The app is ad-hoc signed and distributed as a release download, so
    # Gatekeeper quarantines it and then refuses to open it ("Apple could not
    # verify ... is free of malware"). Stripping the attribute is exactly what
    # the right-click > Open dance does by hand, so `brew install` leaves an
    # app that launches with no extra step.
    #
    # Plain Ruby rather than a cask DSL helper: the block runs in the cask
    # context, which does not expose Homebrew's command wrappers.
    app = File.join(appdir.to_s, "HarryPotter2.app")
    system("/usr/bin/xattr", "-dr", "com.apple.quarantine", app)
  end
end
