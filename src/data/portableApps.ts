// ── Catalogue Applications Portables — agrégateur ───────────────────────────
// Les apps de base sont définies ici. Les extensions par catégorie sont dans
// src/data/portable/cat_*.ts. La déduplication par `id` est automatique.

export interface PortableApp {
  id: string;
  name: string;
  description: string;
  category: string;
  size: string;
  url: string;
  exe_name: string;
  popular?: boolean;
  version?: string;
}

import { CAT_SYSTEME }  from "./portable/cat_systeme";
import { CAT_RESEAU }   from "./portable/cat_reseau";
import { CAT_DEV }      from "./portable/cat_dev";
import { CAT_UTILS }    from "./portable/cat_utils";
import { CAT_MEDIA }    from "./portable/cat_media";
import { CAT_BUREAU }   from "./portable/cat_bureau";
import { CAT_EXTRA }    from "./portable/cat_extra";

// ── Apps de base (230) ───────────────────────────────────────────────────────
const BASE_APPS: PortableApp[] = [
  // ── SYSTÈME ──────────────────────────────────────────────────────────────
  { id:"cpuz",            name:"CPU-Z",               description:"Informations détaillées CPU, carte mère, RAM, SPD",          category:"Système",      size:"3 MB",   url:"https://www.cpuid.com/softwares/cpu-z.html",                         exe_name:"cpuz.exe",            popular:true },
  { id:"gpuz",            name:"GPU-Z",               description:"Informations détaillées carte graphique et VRAM",            category:"Système",      size:"9 MB",   url:"https://www.techpowerup.com/gpuz/",                                  exe_name:"GPU-Z.exe",           popular:true },
  { id:"hwmonitor",       name:"HWMonitor",           description:"Températures, voltages et vitesses de ventilateurs",         category:"Système",      size:"3 MB",   url:"https://www.cpuid.com/softwares/hwmonitor.html",                     exe_name:"HWMonitor.exe",       popular:true },
  { id:"hwinfo64",        name:"HWiNFO64",            description:"Diagnostic hardware complet, capteurs en temps réel",        category:"Système",      size:"12 MB",  url:"https://www.hwinfo.com/download/",                                  exe_name:"HWiNFO64.exe",        popular:true },
  { id:"crystaldiskinfo", name:"CrystalDiskInfo",     description:"Santé des disques S.M.A.R.T., alertes de défaillance",      category:"Système",      size:"6 MB",   url:"https://crystalmark.info/en/software/crystaldiskinfo/",              exe_name:"DiskInfo64.exe",      popular:true },
  { id:"crystaldiskmark", name:"CrystalDiskMark",     description:"Benchmark vitesse lecture/écriture des disques",            category:"Système",      size:"5 MB",   url:"https://crystalmark.info/en/software/crystaldiskmark/",              exe_name:"DiskMark64.exe",      popular:true },
  { id:"speccy",          name:"Speccy",              description:"Vue d'ensemble matériel complète avec températures",         category:"Système",      size:"8 MB",   url:"https://www.ccleaner.com/speccy/download",           exe_name:"Speccy.exe",          popular:true },
  { id:"coretemp",        name:"Core Temp",           description:"Température par cœur CPU, TDP, charge",                     category:"Système",      size:"1 MB",   url:"https://www.alcpu.com/CoreTemp/",                                   exe_name:"Core Temp.exe" },
  { id:"openhardwaremon", name:"Open Hardware Monitor",description:"Moniteur open-source températures, charges CPU/GPU",      category:"Système",      size:"2 MB",   url:"https://openhardwaremonitor.org/downloads/",                        exe_name:"OpenHardwareMonitor.exe" },
  { id:"procexp",         name:"Process Explorer",    description:"Gestionnaire de processus avancé (Sysinternals)",            category:"Système",      size:"4 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/process-explorer",exe_name:"procexp64.exe",       popular:true },
  { id:"procmon",         name:"Process Monitor",     description:"Surveillance activité système temps réel (Sysinternals)",   category:"Système",      size:"3 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/procmon",         exe_name:"Procmon64.exe" },
  { id:"autoruns",        name:"Autoruns",            description:"Gestion avancée du démarrage Windows (Sysinternals)",        category:"Système",      size:"5 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/autoruns",        exe_name:"Autoruns64.exe",      popular:true },
  { id:"winobj",          name:"WinObj",              description:"Visualiseur objets noyau Windows (Sysinternals)",            category:"Système",      size:"1 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/winobj",          exe_name:"WinObj.exe" },
  { id:"sysinternals",    name:"Sysinternals Suite",  description:"Suite complète 70+ outils diagnostic Windows",              category:"Système",      size:"50 MB",  url:"https://learn.microsoft.com/sysinternals/downloads/sysinternals-suite",exe_name:"procexp64.exe",    popular:true },
  { id:"rufus",           name:"Rufus",               description:"Création de clés USB bootables (ISO, Windows, Linux)",      category:"Système",      size:"2 MB",   url:"https://rufus.ie/",                                                 exe_name:"rufus.exe",           popular:true },
  { id:"ventoy",          name:"Ventoy",              description:"Clé USB multiboot, charge directement les ISO",             category:"Système",      size:"15 MB",  url:"https://www.ventoy.net/en/download.html",                           exe_name:"Ventoy2Disk.exe",     popular:true },
  { id:"etcher",          name:"BalenaEtcher",        description:"Graveur d'images disque, interface simple et fiable",        category:"Système",      size:"140 MB", url:"https://etcher.balena.io/",                                         exe_name:"balenaEtcher.exe" },
  { id:"ddu",             name:"DDU",                 description:"Display Driver Uninstaller : suppression propre pilotes GPU",category:"Système",      size:"10 MB",  url:"https://www.guru3d.com/download/display-driver-uninstaller-download/",exe_name:"Display Driver Uninstaller.exe",popular:true },
  { id:"nvclean",         name:"NVCleanstall",        description:"Installation pilotes NVIDIA allégés, sans telemetry",       category:"Système",      size:"5 MB",   url:"https://www.techpowerup.com/nvcleanstall/",                          exe_name:"NVCleanstall.exe" },
  { id:"winget",          name:"WinGet CLI",          description:"Gestionnaire paquets Windows officiel Microsoft",           category:"Système",      size:"5 MB",   url:"https://github.com/microsoft/winget-cli/releases",                  exe_name:"winget.exe",          popular:true },
  { id:"scoop",           name:"Scoop",               description:"Gestionnaire paquets Windows en ligne de commande",         category:"Système",      size:"5 MB",   url:"https://scoop.sh/",                                                 exe_name:"scoop.cmd" },
  { id:"chocolatey",      name:"Chocolatey",          description:"Gestionnaire de paquets Windows, 9000+ logiciels",         category:"Système",      size:"5 MB",   url:"https://chocolatey.org/install",                                    exe_name:"choco.exe",           popular:true },
  // ── RÉSEAU ────────────────────────────────────────────────────────────────
  { id:"wireshark_base",  name:"Wireshark",           description:"Analyseur de protocoles réseau, capture paquets",           category:"Réseau",       size:"80 MB",  url:"https://www.wireshark.org/download.html",                           exe_name:"Wireshark.exe",       popular:true },
  { id:"nmap_base",       name:"Nmap/Zenmap",         description:"Scanner de ports réseau, détection OS et services",         category:"Réseau",       size:"30 MB",  url:"https://nmap.org/download.html",                                    exe_name:"zenmap.exe",          popular:true },
  { id:"putty",           name:"PuTTY",               description:"Client SSH/Telnet/Serial portable, le classique",           category:"Réseau",       size:"3 MB",   url:"https://portableapps.com/apps/internet/putty_portable",             exe_name:"putty.exe",           popular:true },
  { id:"winscp",          name:"WinSCP",              description:"Transfert SFTP/FTP/WebDAV/SCP avec interface graphique",    category:"Réseau",       size:"15 MB",  url:"https://portableapps.com/apps/internet/winscp_portable",            exe_name:"WinSCP.exe",          popular:true },
  { id:"filezilla",       name:"FileZilla",           description:"Client FTP/SFTP/FTPS avec gestion de files de transfert",  category:"Réseau",       size:"20 MB",  url:"https://portableapps.com/apps/internet/filezilla_portable",         exe_name:"filezilla.exe",       popular:true },
  { id:"mremote",         name:"mRemoteNG",           description:"Client RDP/VNC/SSH multi-protocoles, onglets, groupes",    category:"Réseau",       size:"20 MB",  url:"https://mremoteng.org/",                                    exe_name:"mRemoteNG.exe",       popular:true },
  { id:"realvnc",         name:"TightVNC",            description:"Client/serveur VNC portable, partage bureau à distance",   category:"Réseau",       size:"5 MB",   url:"https://portableapps.com/search/node/TightVNC",          exe_name:"tvncviewer.exe" },
  { id:"tcpview",         name:"TCPView",             description:"Connexions TCP/UDP actives temps réel (Sysinternals)",     category:"Réseau",       size:"1 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/tcpview",         exe_name:"Tcpview.exe",         popular:true },
  { id:"networx",         name:"NetWorx",             description:"Moniteur usage bande passante, quotas, graphes",           category:"Réseau",       size:"10 MB",  url:"https://www.softperfect.com/products/networx/",                     exe_name:"networx.exe" },
  { id:"angryip",         name:"Angry IP Scanner",    description:"Scanner IP/ports rapide, hostname, ping, export",          category:"Réseau",       size:"15 MB",  url:"https://angryip.org/download/",                                     exe_name:"ipscan.exe",          popular:true },
  { id:"networktools",    name:"Network Tools",       description:"Suite traceroute, ping, whois, DNS lookup, etc.",           category:"Réseau",       size:"5 MB",   url:"https://portableapps.com/search/node/Network%20Tools",      exe_name:"NetworkTools.exe" },
  // ── DÉVELOPPEMENT ────────────────────────────────────────────────────────
  { id:"vscode",          name:"VS Code",             description:"Éditeur code Microsoft, extensions, débogueur, git",       category:"Développement",size:"100 MB", url:"https://code.visualstudio.com/Download",                            exe_name:"Code.exe",            popular:true },
  { id:"notepadpp",       name:"Notepad++",           description:"Éditeur texte avancé, plugins, macros, regex, portable",   category:"Développement",size:"10 MB",  url:"https://portableapps.com/apps/development/notepadpp_portable",exe_name:"notepad++.exe",       popular:true },
  { id:"sublime",         name:"Sublime Text",        description:"Éditeur code rapide, multi-curseur, projets, plugins",     category:"Développement",size:"20 MB",  url:"https://www.sublimetext.com/download_thanks?target=win-x64",        exe_name:"sublime_text.exe",    popular:true },
  { id:"atom",            name:"Pulsar (Atom fork)",  description:"Éditeur de texte open-source hackable, fork Atom actif",   category:"Développement",size:"200 MB", url:"https://pulsar-edit.dev/download.html",                             exe_name:"Pulsar.exe" },
  { id:"gitbash",         name:"Git for Windows",     description:"Git + Bash pour Windows, MINGW, SSH, GPG, curl",           category:"Développement",size:"50 MB",  url:"https://git-scm.com/download/win",                                  exe_name:"git-bash.exe",        popular:true },
  { id:"github_desktop",  name:"GitHub Desktop",      description:"Interface graphique Git, pull requests, merge conflicts",  category:"Développement",size:"150 MB", url:"https://desktop.github.com/",                                       exe_name:"GitHubDesktop.exe",   popular:true },
  { id:"postman",         name:"Postman",             description:"Test API REST/GraphQL, collections, tests automatisés",    category:"Développement",size:"150 MB", url:"https://www.postman.com/downloads/",                                exe_name:"Postman.exe",         popular:true },
  { id:"insomnia",        name:"Insomnia",            description:"Client REST/GraphQL, environnements, mock servers",        category:"Développement",size:"100 MB", url:"https://insomnia.rest/download",                                    exe_name:"Insomnia.exe",         popular:true },
  { id:"dbeaver",         name:"DBeaver Community",   description:"Client universel DB, 80+ SGBD, SQL editor, ERD, export",  category:"Développement",size:"200 MB", url:"https://dbeaver.io/download/",                                      exe_name:"dbeaver.exe",         popular:true },
  { id:"heidisql",        name:"HeidiSQL",            description:"Client SQL MySQL/MariaDB/PostgreSQL léger, puissant",      category:"Développement",size:"15 MB",  url:"https://www.heidisql.com/download.php",       exe_name:"heidisql.exe",        popular:true },
  { id:"python",          name:"WinPython",           description:"Distribution Python portable Windows, Jupyter, pandas",    category:"Développement",size:"500 MB", url:"https://winpython.github.io/",                                      exe_name:"WinPython.exe",       popular:true },
  { id:"nodejs",          name:"Node.js (portable)",  description:"Runtime JavaScript V8, npm, ES modules",                  category:"Développement",size:"30 MB",  url:"https://nodejs.org/en/download/",                                   exe_name:"node.exe",            popular:true },
  { id:"windows_terminal",name:"Windows Terminal",    description:"Terminal Microsoft moderne, onglets, PowerShell, WSL",    category:"Développement",size:"10 MB",  url:"https://github.com/microsoft/terminal/releases",                    exe_name:"wt.exe",              popular:true },
  // ── UTILITAIRES ──────────────────────────────────────────────────────────
  { id:"7zip",            name:"7-Zip",               description:"Archivage/compression 7z, ZIP, RAR — taux exceptionnel",  category:"Utilitaires",  size:"5 MB",   url:"https://portableapps.com/apps/utilities/7-zip_portable",            exe_name:"7zFM.exe",            popular:true },
  { id:"peazip",          name:"PeaZip",              description:"Archiveur 200+ formats, chiffrement AES-256",             category:"Utilitaires",  size:"12 MB",  url:"https://portableapps.com/apps/utilities/peazip_portable",           exe_name:"peazip.exe" },
  { id:"bandizip",        name:"Bandizip",            description:"Archiveur ultra-rapide multi-format, antivirus intégré",  category:"Utilitaires",  size:"15 MB",  url:"https://www.bandisoft.com/bandizip/",                                exe_name:"Bandizip.exe",        popular:true },
  { id:"eraser",          name:"Eraser",              description:"Effacement sécurisé de fichiers (DoD, Gutmann, PRNG)",   category:"Utilitaires",  size:"20 MB",  url:"https://eraser.heidi.ie/download.aspx",                             exe_name:"Eraser.exe" },
  { id:"filelight",       name:"Filelight",           description:"Visualiseur espace disque radial interactif",             category:"Utilitaires",  size:"10 MB",  url:"https://apps.kde.org/filelight/",                                   exe_name:"filelight.exe" },
  { id:"imageburnz",      name:"ImgBurn",             description:"Gravure/vérification CD/DVD/Blu-ray, création ISO",       category:"Utilitaires",  size:"3 MB",   url:"https://www.imgburn.com/index.php?act=download",                    exe_name:"ImgBurn.exe" },
  { id:"keepass",         name:"KeePass",             description:"Gestionnaire de mots de passe local chiffré AES",         category:"Utilitaires",  size:"4 MB",   url:"https://keepass.info/download.html",           exe_name:"KeePass.exe",         popular:true },
  { id:"greenshot",       name:"Greenshot",           description:"Capture d'écran avancée, annotation, upload direct",      category:"Utilitaires",  size:"3 MB",   url:"https://getgreenshot.org/downloads/",exe_name:"Greenshot.exe",       popular:true },
  { id:"sharex",          name:"ShareX",              description:"Capture/enregistrement écran, 80+ destinations d'upload", category:"Utilitaires",  size:"15 MB",  url:"https://getsharex.com/",                                            exe_name:"ShareX.exe",          popular:true },
  { id:"clover",          name:"Clover",              description:"Ajout d'onglets à l'explorateur Windows",                 category:"Utilitaires",  size:"3 MB",   url:"https://portableapps.com/search/node/Clover",           exe_name:"Clover.exe" },
  { id:"launchy",         name:"Launchy",             description:"Lanceur d'applications par frappe clavier",              category:"Utilitaires",  size:"8 MB",   url:"https://portableapps.com/search/node/Launchy",          exe_name:"Launchy.exe" },
  { id:"cliptray",        name:"ClipTray",            description:"Gestionnaire presse-papiers avec historique illimité",   category:"Utilitaires",  size:"2 MB",   url:"https://portableapps.com/search/node/ClipTray",         exe_name:"ClipTray.exe" },
  { id:"nanazip",         name:"NanaZip",             description:"Archiveur moderne basé sur 7-Zip, intégration Windows 11",category:"Utilitaires", size:"6 MB",   url:"https://github.com/M2Team/NanaZip/releases",                        exe_name:"NanaZip.exe" },
  { id:"recuva",          name:"Recuva",              description:"Récupération fichiers supprimés ou formatés",             category:"Utilitaires",  size:"8 MB",   url:"https://portableapps.com/apps/utilities/recuva_portable",           exe_name:"Recuva.exe",          popular:true },
  { id:"testdisk",        name:"TestDisk",            description:"Récupération partitions, table MFT, démarrage perdu",    category:"Utilitaires",  size:"3 MB",   url:"https://www.cgsecurity.org/wiki/TestDisk_Download",                 exe_name:"testdisk_win.exe",    popular:true },
  { id:"photorec",        name:"PhotoRec",            description:"Récupération photos, vidéos et fichiers sur tout support",category:"Utilitaires",  size:"3 MB",   url:"https://www.cgsecurity.org/wiki/TestDisk_Download",                 exe_name:"photorec_win.exe",    popular:true },
  { id:"bulkrenamer",     name:"Bulk Rename Utility", description:"Renommage en masse de fichiers avec regex, dates, EXIF", category:"Utilitaires",  size:"7 MB",   url:"https://www.bulkrenameutility.co.uk/Download.php",                  exe_name:"BulkRenameUtility.exe",popular:true },
  { id:"hashfiles",       name:"HashMyFiles",         description:"Calcul MD5/SHA1/SHA256/CRC32 de fichiers",               category:"Utilitaires",  size:"1 MB",   url:"https://www.nirsoft.net/utils/hash_my_files.html",                  exe_name:"HashMyFiles.exe" },
  { id:"virtualclonedrive",name:"Virtual CloneDrive", description:"Montage d'images ISO/IMG/BIN comme lecteur virtuel",     category:"Utilitaires",  size:"5 MB",   url:"https://www.elby.ch/en/products/vcd.html",                          exe_name:"VCDPortable.exe" },
  // ── MULTIMÉDIA ────────────────────────────────────────────────────────────
  { id:"vlc",             name:"VLC",                 description:"Lecteur multimédia universel, tous formats audio/vidéo", category:"Multimédia",   size:"40 MB",  url:"https://portableapps.com/apps/music_video/vlc_portable",            exe_name:"VLC.exe",             popular:true },
  { id:"mpchc",           name:"MPC-HC",              description:"Lecteur vidéo léger et rapide, sous-titres intégrés",    category:"Multimédia",   size:"15 MB",  url:"https://github.com/clsid2/mpc-hc/releases",                         exe_name:"mpc-hc64.exe",        popular:true },
  { id:"mpv",             name:"mpv",                 description:"Lecteur vidéo en ligne de commande ultra-performant",    category:"Multimédia",   size:"30 MB",  url:"https://mpv.io/installation/",                                      exe_name:"mpv.exe" },
  { id:"foobar2000",      name:"foobar2000",          description:"Lecteur audio professionnel, DSP, plugins, FLAC/OGG",   category:"Multimédia",   size:"10 MB",  url:"https://www.foobar2000.org/download",     exe_name:"foobar2000.exe",      popular:true },
  { id:"aimp",            name:"AIMP",                description:"Lecteur audio avancé, égaliseur 29 bandes, DSP",         category:"Multimédia",   size:"20 MB",  url:"https://www.aimp.ru/",                                              exe_name:"AIMP.exe",            popular:true },
  { id:"audacity",        name:"Audacity",            description:"Éditeur audio open-source, enregistrement, effets",      category:"Multimédia",   size:"30 MB",  url:"https://portableapps.com/apps/music_video/audacity_portable",       exe_name:"Audacity.exe",        popular:true },
  { id:"mp3tag",          name:"Mp3tag",              description:"Éditeur de tags audio universel, couvertures, MusicBrainz",category:"Multimédia", size:"10 MB",  url:"https://www.mp3tag.de/en/download.html",         exe_name:"Mp3tag.exe",          popular:true },
  { id:"mediainfo",       name:"MediaInfo",           description:"Informations techniques audio/vidéo : codec, débit, etc.",category:"Multimédia",  size:"5 MB",   url:"https://mediaarea.net/fr/MediaInfo/Download/Windows",               exe_name:"MediaInfo.exe",       popular:true },
  { id:"handbrake",       name:"HandBrake",           description:"Conversion vidéo open-source, H.264/265, tous formats",  category:"Multimédia",   size:"15 MB",  url:"https://handbrake.fr/downloads.php",      exe_name:"HandBrake.exe",       popular:true },
  { id:"virtualdub2",     name:"VirtualDub2",         description:"Édition vidéo légère, filtres, AVI/MP4",                 category:"Multimédia",   size:"10 MB",  url:"https://virtualdub2.com/",                                          exe_name:"VirtualDub2.exe" },
  { id:"obs",             name:"OBS Studio",          description:"Capture/streaming open-source, scènes, filtres",          category:"Multimédia",   size:"300 MB", url:"https://obsproject.com/fr/download",                                exe_name:"obs64.exe",           popular:true },
  { id:"ytdlp",           name:"yt-dlp",              description:"Téléchargement vidéos YouTube, Twitch, 1000+ sites",     category:"Multimédia",   size:"15 MB",  url:"https://github.com/yt-dlp/yt-dlp/releases",                         exe_name:"yt-dlp.exe",          popular:true },
  { id:"ffmpeg",          name:"FFmpeg",              description:"Conversion/traitement audio-vidéo en ligne de commande", category:"Multimédia",   size:"80 MB",  url:"https://ffmpeg.org/download.html",                                  exe_name:"ffmpeg.exe",          popular:true },
  { id:"musicbee",        name:"MusicBee",            description:"Gestion bibliothèque musicale, podcasts, synchronisation",category:"Multimédia",   size:"20 MB",  url:"https://getmusicbee.com/downloads/",                                exe_name:"MusicBee.exe" },
  { id:"clementine",      name:"Clementine",          description:"Lecteur musique, Spotify, Subsonic, lyrics automatiques",category:"Multimédia",   size:"30 MB",  url:"https://portableapps.com/search/node/Clementine",     exe_name:"clementine.exe" },
  // ── BUREAUTIQUE ───────────────────────────────────────────────────────────
  { id:"libreoffice",     name:"LibreOffice",         description:"Suite bureautique complète : Writer, Calc, Impress",      category:"Bureautique",  size:"350 MB", url:"https://portableapps.com/apps/office/libreoffice_portable",         exe_name:"soffice.exe",         popular:true },
  { id:"sumatrapdf",      name:"SumatraPDF",          description:"Lecteur PDF/EPUB/CBZ ultra-léger, raccourcis clavier",    category:"Bureautique",  size:"7 MB",   url:"https://portableapps.com/apps/office/sumatra_pdf_portable",         exe_name:"SumatraPDF.exe",      popular:true },
  { id:"pdf24",           name:"PDF24 Creator",       description:"Création/fusion/compression PDF, OCR, signature",         category:"Bureautique",  size:"50 MB",  url:"https://tools.pdf24.org/fr/pdf24-creator",                          exe_name:"pdf24-creator.exe",   popular:true },
  { id:"adobereader",     name:"Foxit PDF Reader",    description:"Lecteur PDF léger, annotations, formulaires",             category:"Bureautique",  size:"60 MB",  url:"https://portableapps.com/apps/office/foxit_reader_portable",        exe_name:"FoxitPDFReader.exe",  popular:true },
  { id:"calibre",         name:"Calibre",             description:"Gestion ebooks, conversion EPUB/MOBI/PDF, éditeur",       category:"Bureautique",  size:"160 MB", url:"https://calibre-ebook.com/download_portable",             exe_name:"calibre.exe",         popular:true },
  { id:"joplin",          name:"Joplin",              description:"Prise de notes Markdown, synchronisation, chiffrement",   category:"Bureautique",  size:"200 MB", url:"https://joplinapp.org/download/",                                   exe_name:"Joplin.exe",          popular:true },
  { id:"obsidian",        name:"Obsidian",            description:"Notes interconnectées Markdown, graphe de connaissances", category:"Bureautique",  size:"120 MB", url:"https://obsidian.md/download",                                      exe_name:"Obsidian.exe",        popular:true },
  { id:"notepadplusx",    name:"Notepad2",            description:"Remplacement Bloc-notes Windows, colorisation syntaxique",category:"Bureautique",  size:"2 MB",   url:"https://portableapps.com/search/node/Notepad2",        exe_name:"Notepad2.exe" },
  { id:"focuswriter",     name:"FocusWriter",         description:"Traitement de texte plein écran sans distraction",        category:"Bureautique",  size:"20 MB",  url:"https://portableapps.com/apps/office/focuswriter_portable",         exe_name:"FocusWriter.exe" },
  { id:"zotero",          name:"Zotero",              description:"Gestion références bibliographiques, import PDF",          category:"Bureautique",  size:"80 MB",  url:"https://www.zotero.org/download/",                                  exe_name:"zotero.exe" },
  { id:"keepnote",        name:"CherryTree",          description:"Bloc-notes hiérarchique, code coloré, chiffrement",       category:"Bureautique",  size:"30 MB",  url:"https://portableapps.com/search/node/CherryTree",          exe_name:"cherrytree.exe",      popular:true },
  { id:"scribus",         name:"Scribus",             description:"PAO/DTP open-source, mise en page professionnelle",        category:"Bureautique",  size:"120 MB", url:"https://portableapps.com/apps/office/scribus_portable",             exe_name:"Scribus.exe" },
  // ── NETTOYAGE ─────────────────────────────────────────────────────────────
  { id:"bleachbit",       name:"BleachBit",           description:"Nettoyeur système open-source, 70+ applications",         category:"Nettoyage",    size:"15 MB",  url:"https://portableapps.com/apps/utilities/bleachbit_portable",        exe_name:"bleachbit.exe",       popular:true },
  { id:"adwcleaner",      name:"AdwCleaner",          description:"Suppression adwares, toolbars, PUP, hijackers",           category:"Nettoyage",    size:"8 MB",   url:"https://www.malwarebytes.com/adwcleaner",                           exe_name:"adwcleaner.exe",      popular:true },
  { id:"ccleaner",        name:"CCleaner",            description:"Nettoyage registre, cache, navigation, démarrage",        category:"Nettoyage",    size:"25 MB",  url:"https://www.ccleaner.com/ccleaner/builds",         exe_name:"CCleaner.exe",        popular:true },
  { id:"wisecare365",     name:"Wise Care 365",       description:"Optimisation système complète, nettoyage, boost démarrage",category:"Nettoyage",   size:"40 MB",  url:"https://portableapps.com/search/node/Wise%20Care%20365",   exe_name:"WiseCare365.exe",     popular:true },
  { id:"wisedisk",        name:"Wise Disk Cleaner",   description:"Nettoyage fichiers temporaires, défrag, analyse disque",  category:"Nettoyage",    size:"15 MB",  url:"https://portableapps.com/search/node/Wise%20Disk%20Cleaner",exe_name:"WiseDiskCleaner.exe" },
  { id:"privazer",        name:"PrivaZer",            description:"Nettoyage traces confidentielles, effacement sécurisé",   category:"Nettoyage",    size:"15 MB",  url:"https://privazer.com/fr/download.php",                              exe_name:"PrivaZer.exe",        popular:true },
  { id:"glary",           name:"Glary Utilities",     description:"Suite optimisation 20+ outils : registre, RAM, startup",  category:"Nettoyage",    size:"30 MB",  url:"https://portableapps.com/search/node/Glary%20Utilities",  exe_name:"Integrator.exe" },
  { id:"revo",            name:"Revo Uninstaller",    description:"Désinstallation propre avec nettoyage des résidus",        category:"Nettoyage",    size:"15 MB",  url:"https://portableapps.com/apps/utilities/revo_uninstaller_portable", exe_name:"RevoUninstaller.exe", popular:true },
  { id:"iobit",           name:"IObit Uninstaller",   description:"Désinstallation forcée + nettoyage résidus registre",     category:"Nettoyage",    size:"30 MB",  url:"https://portableapps.com/apps/utilities/iobit_uninstaller_portable",exe_name:"IObitUninstaler.exe" },
  { id:"autoruns_clean",  name:"Autoruns Clean",      description:"Désactiver/supprimer entrées de démarrage parasites",     category:"Nettoyage",    size:"5 MB",   url:"https://learn.microsoft.com/sysinternals/downloads/autoruns",       exe_name:"Autoruns.exe" },
  // ── SÉCURITÉ ──────────────────────────────────────────────────────────────
  { id:"malwarebytes",    name:"Malwarebytes",        description:"Scan et suppression malwares, ransomwares, PUP",          category:"Sécurité",     size:"70 MB",  url:"https://www.malwarebytes.com/mwb-download/thankyou",                exe_name:"mbam.exe",            popular:true },
  { id:"kaspersky_vrt",   name:"Kaspersky KVRT",      description:"Outil de suppression virus Kaspersky sans installation",  category:"Sécurité",     size:"200 MB", url:"https://www.kaspersky.fr/downloads/free-virus-removal-tool",        exe_name:"KVRT.exe",            popular:true },
  { id:"drweb",           name:"Dr.Web CureIt!",      description:"Scanner antivirus d'urgence Dr.Web sans installation",    category:"Sécurité",     size:"170 MB", url:"https://free.drweb.fr/download+cureit/",                            exe_name:"drweb-cureit.exe",    popular:true },
  { id:"emsisoft",        name:"Emsisoft Emergency",  description:"Scanner anti-malware d'urgence offline",                  category:"Sécurité",     size:"700 MB", url:"https://www.emsisoft.com/",                   exe_name:"EmsisoftEmergencyKit.exe",popular:true },
  { id:"hijackthis",      name:"HiJackThis Fork",     description:"Analyse et suppression hijackers navigateur, BHO, hosts",category:"Sécurité",     size:"2 MB",   url:"https://github.com/dragokas/hijackthis/releases",                   exe_name:"HiJackThis.exe" },
  { id:"rkill",           name:"RKill",               description:"Tue les processus malveillants pour permettre le scan",   category:"Sécurité",     size:"2 MB",   url:"https://www.bleepingcomputer.com/download/rkill/",                  exe_name:"rkill.exe",           popular:true },
  { id:"roguekiller",     name:"RogueKiller",         description:"Détection et suppression rogues, rootkits, FakeAV",       category:"Sécurité",     size:"60 MB",  url:"https://www.adlice.com/roguekiller/",                               exe_name:"RogueKiller.exe",     popular:true },
  { id:"veracrypt",       name:"VeraCrypt",           description:"Chiffrement volumes/partitions AES, deux niveaux de clé", category:"Sécurité",     size:"30 MB",  url:"https://www.veracrypt.fr/en/Downloads.html",                        exe_name:"VeraCrypt.exe",       popular:true },
  { id:"cryptomator",     name:"Cryptomator",         description:"Chiffrement cloud transparent, coffres-forts virtuels",   category:"Sécurité",     size:"100 MB", url:"https://cryptomator.org/downloads/",                                exe_name:"Cryptomator.exe" },
  { id:"clamwin",         name:"ClamWin Portable",    description:"Antivirus open-source ClamAV pour Windows",               category:"Sécurité",     size:"60 MB",  url:"https://portableapps.com/apps/security/clamwin_portable",           exe_name:"ClamWin.exe" },
  { id:"processhacker",   name:"Process Hacker",      description:"Moniteur système avancé, mémoire, réseau, injections",   category:"Sécurité",     size:"10 MB",  url:"https://processhacker.sourceforge.io/downloads.php",                exe_name:"ProcessHacker.exe",   popular:true },
  { id:"gmer",            name:"GMER",                description:"Scanner rootkits kernel, hooks, ADS, services cachés",   category:"Sécurité",     size:"1 MB",   url:"https://www.gmer.net/",                                             exe_name:"gmer.exe" },
  { id:"spybot",          name:"Spybot S&D",          description:"Détection spywares, immunisation navigateurs, hosts",     category:"Sécurité",     size:"80 MB",  url:"https://portableapps.com/apps/security/spybot_portable",            exe_name:"SpybotSD.exe" },
  { id:"mcafee_stinger",  name:"McAfee Stinger",      description:"Outil suppression virus ciblés McAfee, gratuit",          category:"Sécurité",     size:"60 MB",  url:"https://www.trellix.com/en-us/downloads/free-tools/stinger.html",  exe_name:"stinger.exe" },
  { id:"keepassxc",       name:"KeePassXC",           description:"Fork KeePass moderne, TOTP, SSH agent, auto-type",        category:"Sécurité",     size:"30 MB",  url:"https://keepassxc.org/download/",                                   exe_name:"KeePassXC.exe",       popular:true },
  { id:"bitwarden",       name:"Bitwarden Desktop",   description:"Gestionnaire mots de passe cloud + local, open-source",  category:"Sécurité",     size:"100 MB", url:"https://bitwarden.com/download/",                                   exe_name:"Bitwarden.exe",       popular:true },
  // ── GRAPHISME ─────────────────────────────────────────────────────────────
  { id:"gimp",            name:"GIMP",                description:"Éditeur d'images avancé, calques, scripts, plugins",      category:"Graphisme",    size:"250 MB", url:"https://portableapps.com/apps/graphics_pictures/gimp_portable",     exe_name:"gimp.exe",            popular:true },
  { id:"inkscape",        name:"Inkscape",            description:"Éditeur vectoriel SVG professionnel open-source",          category:"Graphisme",    size:"200 MB", url:"https://portableapps.com/apps/graphics_pictures/inkscape_portable", exe_name:"inkscape.exe",        popular:true },
  { id:"irfanview",       name:"IrfanView",           description:"Visionneuse d'images ultra-rapide, batch, conversion",    category:"Graphisme",    size:"4 MB",   url:"https://portableapps.com/apps/graphics_pictures/irfanview_portable",exe_name:"i_view64.exe",        popular:true },
  { id:"faststone",       name:"FastStone Image",     description:"Visionneuse/éditeur images, effets, batch, comparaison",  category:"Graphisme",    size:"8 MB",   url:"https://portableapps.com/search/node/FastStone%20Image",exe_name:"FSViewer.exe",popular:true },
  { id:"xnviewmp",        name:"XnView MP",           description:"Visualiseur 500+ formats, gestion catalogue, EXIF",       category:"Graphisme",    size:"50 MB",  url:"https://www.xnview.com/fr/xnviewmp/",                               exe_name:"xnviewmp.exe",        popular:true },
  { id:"krita",           name:"Krita",               description:"Peinture numérique professionnelle, brosse HDR, animations",category:"Graphisme",  size:"250 MB", url:"https://krita.org/fr/telechargement/",                              exe_name:"krita.exe",           popular:true },
  { id:"darktable",       name:"darktable",           description:"Développement RAW, chambre noire numérique",               category:"Graphisme",    size:"200 MB", url:"https://www.darktable.org/install/",                                exe_name:"darktable.exe" },
  { id:"rawtherapee",     name:"RawTherapee",         description:"Traitement RAW avancé, couleurs, bruit, netteté",          category:"Graphisme",    size:"120 MB", url:"https://rawtherapee.com/downloads",                                 exe_name:"rawtherapee.exe" },
  { id:"exiftool",        name:"ExifTool",            description:"Lecture/écriture métadonnées EXIF/XMP/IPTC de tous médias",category:"Graphisme",    size:"5 MB",   url:"https://exiftool.org/",                                             exe_name:"exiftool.exe",        popular:true },
  { id:"digikam",         name:"digiKam",             description:"Gestion photo professionnelle, catalogue, AI, retouche",  category:"Graphisme",    size:"300 MB", url:"https://www.digikam.org/download/",                                 exe_name:"digikam.exe" },
  { id:"hugin",           name:"Hugin",               description:"Panoramique et assemblage photos, correction distorsion",  category:"Graphisme",    size:"80 MB",  url:"https://portableapps.com/search/node/Hugin",   exe_name:"hugin.exe" },
  { id:"imagemagick",     name:"ImageMagick",         description:"Traitement images en ligne de commande, batch illimité",  category:"Graphisme",    size:"50 MB",  url:"https://imagemagick.org/script/download.php",                       exe_name:"magick.exe",          popular:true },
  { id:"paint_net",       name:"Paint.NET",           description:"Éditeur image Windows simple mais puissant, plugins",     category:"Graphisme",    size:"15 MB",  url:"https://www.getpaint.net/download.html",                            exe_name:"PaintDotNet.exe",     popular:true },
  { id:"blender",         name:"Blender",             description:"Modélisation 3D, animation, rendu, VFX, open-source",     category:"Graphisme",    size:"400 MB", url:"https://www.blender.org/download/",                                 exe_name:"blender.exe",         popular:true },
  { id:"figma",           name:"Figma Desktop",       description:"Design UI/UX collaboratif, prototypage, composants",      category:"Graphisme",    size:"200 MB", url:"https://www.figma.com/downloads/",                                  exe_name:"Figma.exe",           popular:true },
  // ── RÉCUPÉRATION ──────────────────────────────────────────────────────────
  { id:"minitool_part",   name:"MiniTool Partition",  description:"Gestion partitions, redimensionnement, migration OS",     category:"Récupération", size:"80 MB",  url:"https://www.partitionwizard.com/free-partition-manager.html",       exe_name:"PartitionWizard.exe", popular:true },
  { id:"easeus_part",     name:"EaseUS Partition",    description:"Partition Manager : créer, fusionner, redimensionner",    category:"Récupération", size:"90 MB",  url:"https://www.easeus.com/partition-manager/epm-free.html",            exe_name:"EaseUS Partition Master.exe" },
  { id:"testdisk_r",      name:"TestDisk / PhotoRec", description:"Récupération partitions perdues et fichiers supprimés",   category:"Récupération", size:"3 MB",   url:"https://www.cgsecurity.org/wiki/TestDisk_Download",                 exe_name:"testdisk_win.exe",    popular:true },
  { id:"recuva_r",        name:"Recuva Pro",          description:"Récupération fichiers formatés, cartes SD, clés USB",     category:"Récupération", size:"8 MB",   url:"https://portableapps.com/apps/utilities/recuva_portable",           exe_name:"Recuva64.exe",        popular:true },
  { id:"diskdrillp",      name:"Disk Drill",          description:"Récupération données multi-format, prévisualisation",     category:"Récupération", size:"120 MB", url:"https://www.cleverfiles.com/fr/disk-drill-windows.html",            exe_name:"DiskDrill.exe" },
  { id:"getdataback",     name:"GetDataBack",         description:"Récupération NTFS/FAT, secteurs défectueux, corruption",  category:"Récupération", size:"8 MB",   url:"https://www.runtime.org/data-recovery-software.htm",               exe_name:"GetDataBackNew.exe",  popular:true },
  { id:"hddscan",         name:"HDDScan",             description:"Diagnostic HDD/SSD : S.M.A.R.T., test surface, AAM",     category:"Récupération", size:"4 MB",   url:"https://hddscan.com/",                                              exe_name:"HDDScan.exe",         popular:true },
  { id:"victoria",        name:"Victoria",            description:"Test disque bas niveau, réparation secteurs, S.M.A.R.T.", category:"Récupération", size:"2 MB",   url:"https://hdd.by/victoria/",                                          exe_name:"victoria.exe",        popular:true },
  { id:"hd_tune",         name:"HD Tune Pro",         description:"Benchmark disque, scan d'erreurs, santé SMART",           category:"Récupération", size:"5 MB",   url:"https://www.hdtune.com/",                                           exe_name:"HD Tune.exe" },
  { id:"driveimagexml",   name:"DriveImage XML",      description:"Clonage disque/partition, sauvegarde image, restauration",category:"Récupération", size:"10 MB",  url:"https://www.runtime.org/driveimage-xml.htm",                        exe_name:"DriveImageXML.exe" },
  { id:"ntpasswordedit",  name:"NT Password Edit",    description:"Réinitialisation mot de passe Windows hors-ligne",        category:"Récupération", size:"1 MB",   url:"https://www.cdslow.org.ru/en/ntpwedit/",                            exe_name:"ntpwedit.exe" },
  { id:"hiren_boot",      name:"Hiren's BootCD PE",   description:"CD/USB de secours Windows PE avec 100+ outils",          category:"Récupération", size:"2 GB",   url:"https://www.hirensbootcd.org/download/",                            exe_name:"hirens.exe",          popular:true },
];

// ── Fusion + déduplication par id ────────────────────────────────────────────
const ALL_SOURCES: PortableApp[] = [
  ...BASE_APPS,
  ...CAT_SYSTEME,
  ...CAT_RESEAU,
  ...CAT_DEV,
  ...CAT_UTILS,
  ...CAT_MEDIA,
  ...CAT_BUREAU,
  ...CAT_EXTRA,
];

const _seen = new Set<string>();
export const PORTABLE_APPS: PortableApp[] = ALL_SOURCES.filter(app => {
  if (_seen.has(app.id)) return false;
  _seen.add(app.id);
  return true;
});

export const PORTABLE_CATEGORIES = [
  "Tous",
  "Système",
  "Réseau",
  "Développement",
  "Utilitaires",
  "Multimédia",
  "Bureautique",
  "Nettoyage",
  "Sécurité",
  "Graphisme",
  "Récupération",
] as const;
