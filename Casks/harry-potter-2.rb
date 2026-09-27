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
  # The engine is built arm64-only, and CMake refuses to configure anywhere
  # else. Fail the install on an Intel Mac instead of shipping an app that
  # cannot launch.
  depends_on arch: :arm64

  # Gatekeeper note, for both install routes.
  #
  # This app is ad-hoc signed and arrives as a download, so macOS quarantines
  # it and then refuses to open it ("Apple could not verify ... is free of
  # malware"). Clearing the attribute is exactly what the right-click > Open
  # dance does by hand. If that fails the install must not report success
  # while leaving an app the user cannot launch, so this aborts.
  #
  # Plain Ruby: the block runs in the cask context, which does not expose
  # Homebrew's command wrappers.
  postflight do
    target = File.join(appdir.to_s, "HarryPotter2.app")
    unless system("/usr/bin/xattr", "-dr", "com.apple.quarantine", target)
      abort "could not clear the quarantine attribute on #{target}; macOS " \
            "will block the app on first launch. Open it once via " \
            "right-click > Open, or run: " \
            "xattr -dr com.apple.quarantine #{target}"
    end
  end
end
