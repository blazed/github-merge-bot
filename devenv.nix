{
  pkgs,
  config,
  ...
}: {
  env.LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

  languages.rust = {
    enable = true;
    channel = "nightly";
    components = ["rustc" "cargo" "clippy" "rustfmt" "rust-analyzer"];
  };

  languages.nix.enable = true;
  git-hooks = {
    hooks = {
      cargo-check.enable = true;
      clippy = {
        enable = true;
        settings.denyWarnings = true;
      };
      rustfmt.enable = true;
      rustfmt.packageOverrides.rustfmt = config.languages.rust.toolchain.rustfmt;
    };
    settings.rust.cargoManifestPath = "./Cargo.toml";
  };
  dotenv.enable = true;

  services.cockroachdb.enable = true;

  packages = with pkgs; [
    alejandra
    cargo-deny
    cargo-machete
    just
    sqlx-cli
  ];

  scripts.cargofmt.exec = ''
    cargo fmt --all -- --check
  '';

  scripts.clippy.exec = ''
    cargo clippy --workspace --tests --bins --lib -- -D warnings
  '';

  enterShell = ''
    echo -e "
      GithHub merge bot
    "
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
  '';

  enterTest = ''
    wait_for_port 8080
    echo "Running tests"
    cargo test --workspace
  '';
}
