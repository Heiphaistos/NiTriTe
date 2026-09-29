//! Ressources Windows de l'executable : icone NiTriTe, informations de
//! version (Proprietes > Details), et manifeste qui demande les droits
//! administrateur au lancement, comme NiTriTe (DISM, SFC, installations…).

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="NiTriTe.Agent" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>"#;

fn main() {
    // Version du backend NiTriTe embarque, affichee dans le menu « Agent ».
    let core = std::fs::read_to_string("../core/Cargo.toml").unwrap_or_default();
    let version = core
        .lines()
        .find_map(|l| l.strip_prefix("version = \"").and_then(|v| v.strip_suffix('"')))
        .unwrap_or("?");
    println!("cargo:rustc-env=NITRITE_CORE_VERSION={version}");
    println!("cargo:rerun-if-changed=../core/Cargo.toml");
    println!("cargo:rerun-if-changed=assets/nitrite-agent.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/nitrite-agent.ico")
            .set("ProductName", "NiTriTe Agent")
            .set("FileDescription", "NiTriTe Agent — panneau web de NiTriTe")
            .set("CompanyName", "NiTriTe")
            .set("LegalCopyright", "MIT")
            .set("OriginalFilename", "NiTriTe-Agent.exe")
            .set_manifest(MANIFEST);
        res.compile().expect("ressources Windows (icone, version, manifeste)");
    }
}
