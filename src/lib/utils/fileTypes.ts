export interface FileTypeInfo {
  icon: string;
  color: string;
  label: string;
}

const fileTypeMap: Record<string, FileTypeInfo> = {
  // Documents
  pdf: { icon: 'document', color: '#e74c3c', label: 'PDF' },
  doc: { icon: 'document', color: '#2b579a', label: 'Word' },
  docx: { icon: 'document', color: '#2b579a', label: 'Word' },
  xls: { icon: 'document', color: '#217346', label: 'Excel' },
  xlsx: { icon: 'document', color: '#217346', label: 'Excel' },
  ppt: { icon: 'document', color: '#d24726', label: 'PowerPoint' },
  pptx: { icon: 'document', color: '#d24726', label: 'PowerPoint' },
  odt: { icon: 'document', color: '#2b579a', label: 'Document' },
  rtf: { icon: 'document', color: '#666666', label: 'RTF' },
  
  // Text
  txt: { icon: 'file-text', color: '#95a5a6', label: 'Text' },
  md: { icon: 'file-text', color: '#083fa1', label: 'Markdown' },
  markdown: { icon: 'file-text', color: '#083fa1', label: 'Markdown' },
  json: { icon: 'code', color: '#f7df1e', label: 'JSON' },
  xml: { icon: 'code', color: '#e34f26', label: 'XML' },
  html: { icon: 'code', color: '#e34f26', label: 'HTML' },
  htm: { icon: 'code', color: '#e34f26', label: 'HTML' },
  csv: { icon: 'file-text', color: '#27ae60', label: 'CSV' },
  
  // Code
  js: { icon: 'code', color: '#f7df1e', label: 'JavaScript' },
  ts: { icon: 'code', color: '#3178c6', label: 'TypeScript' },
  jsx: { icon: 'code', color: '#61dafb', label: 'React' },
  tsx: { icon: 'code', color: '#61dafb', label: 'React' },
  vue: { icon: 'code', color: '#42b883', label: 'Vue' },
  svelte: { icon: 'code', color: '#ff3e00', label: 'Svelte' },
  py: { icon: 'code', color: '#3776ab', label: 'Python' },
  rb: { icon: 'code', color: '#cc342d', label: 'Ruby' },
  go: { icon: 'code', color: '#00add8', label: 'Go' },
  rs: { icon: 'code', color: '#dea584', label: 'Rust' },
  java: { icon: 'code', color: '#007396', label: 'Java' },
  kt: { icon: 'code', color: '#7f52ff', label: 'Kotlin' },
  c: { icon: 'code', color: '#555555', label: 'C' },
  cpp: { icon: 'code', color: '#00599c', label: 'C++' },
  h: { icon: 'code', color: '#555555', label: 'Header' },
  hpp: { icon: 'code', color: '#00599c', label: 'Header' },
  cs: { icon: 'code', color: '#239120', label: 'C#' },
  php: { icon: 'code', color: '#777bb4', label: 'PHP' },
  swift: { icon: 'code', color: '#f05138', label: 'Swift' },
  sql: { icon: 'database', color: '#e38c00', label: 'SQL' },
  sh: { icon: 'code', color: '#4eaa25', label: 'Shell' },
  bash: { icon: 'code', color: '#4eaa25', label: 'Bash' },
  zsh: { icon: 'code', color: '#4eaa25', label: 'Zsh' },
  ps1: { icon: 'code', color: '#012456', label: 'PowerShell' },
  yaml: { icon: 'code', color: '#cb171e', label: 'YAML' },
  yml: { icon: 'code', color: '#cb171e', label: 'YAML' },
  toml: { icon: 'code', color: '#9c4221', label: 'TOML' },
  ini: { icon: 'code', color: '#6e6e6e', label: 'INI' },
  cfg: { icon: 'code', color: '#6e6e6e', label: 'Config' },
  conf: { icon: 'code', color: '#6e6e6e', label: 'Config' },
  
  // Images
  png: { icon: 'image', color: '#a855f7', label: 'PNG' },
  jpg: { icon: 'image', color: '#a855f7', label: 'JPEG' },
  jpeg: { icon: 'image', color: '#a855f7', label: 'JPEG' },
  gif: { icon: 'image', color: '#a855f7', label: 'GIF' },
  svg: { icon: 'image', color: '#ffb13b', label: 'SVG' },
  webp: { icon: 'image', color: '#a855f7', label: 'WebP' },
  bmp: { icon: 'image', color: '#a855f7', label: 'BMP' },
  ico: { icon: 'image', color: '#a855f7', label: 'Icon' },
  psd: { icon: 'image', color: '#31a8ff', label: 'Photoshop' },
  ai: { icon: 'image', color: '#ff9a00', label: 'Illustrator' },
  
  // Video
  mp4: { icon: 'video', color: '#ec4899', label: 'MP4' },
  mov: { icon: 'video', color: '#ec4899', label: 'QuickTime' },
  avi: { icon: 'video', color: '#ec4899', label: 'AVI' },
  mkv: { icon: 'video', color: '#ec4899', label: 'MKV' },
  webm: { icon: 'video', color: '#ec4899', label: 'WebM' },
  flv: { icon: 'video', color: '#ec4899', label: 'FLV' },
  wmv: { icon: 'video', color: '#ec4899', label: 'WMV' },
  
  // Audio
  mp3: { icon: 'music', color: '#8b5cf6', label: 'MP3' },
  wav: { icon: 'music', color: '#8b5cf6', label: 'WAV' },
  flac: { icon: 'music', color: '#8b5cf6', label: 'FLAC' },
  aac: { icon: 'music', color: '#8b5cf6', label: 'AAC' },
  ogg: { icon: 'music', color: '#8b5cf6', label: 'OGG' },
  m4a: { icon: 'music', color: '#8b5cf6', label: 'M4A' },
  
  // Archives
  zip: { icon: 'archive', color: '#f59e0b', label: 'ZIP' },
  rar: { icon: 'archive', color: '#f59e0b', label: 'RAR' },
  '7z': { icon: 'archive', color: '#f59e0b', label: '7-Zip' },
  tar: { icon: 'archive', color: '#f59e0b', label: 'TAR' },
  gz: { icon: 'archive', color: '#f59e0b', label: 'GZ' },
  bz2: { icon: 'archive', color: '#f59e0b', label: 'BZ2' },
  xz: { icon: 'archive', color: '#f59e0b', label: 'XZ' },
  
  // Executables
  exe: { icon: 'hard-drive', color: '#ef4444', label: 'Executable' },
  msi: { icon: 'hard-drive', color: '#ef4444', label: 'Installer' },
  dmg: { icon: 'hard-drive', color: '#6b7280', label: 'DMG' },
  deb: { icon: 'hard-drive', color: '#a855f7', label: 'Debian' },
  rpm: { icon: 'hard-drive', color: '#f97316', label: 'RPM' },
  app: { icon: 'hard-drive', color: '#6b7280', label: 'App' },
  
  // Data
  db: { icon: 'database', color: '#f97316', label: 'Database' },
  sqlite: { icon: 'database', color: '#f97316', label: 'SQLite' },
};

export function getFileTypeInfo(filename: string): FileTypeInfo {
  const ext = filename.split('.').pop()?.toLowerCase() || '';
  return fileTypeMap[ext] || { icon: 'file', color: '#6b7280', label: 'File' };
}

export function getFileIcon(filename: string): string {
  return getFileTypeInfo(filename).icon;
}

export function getFileColor(filename: string): string {
  return getFileTypeInfo(filename).color;
}

export function getFileLabel(filename: string): string {
  return getFileTypeInfo(filename).label;
}

export const fileTypeColors = {
  document: '#2b579a',
  text: '#95a5a6',
  code: '#3178c6',
  image: '#a855f7',
  video: '#ec4899',
  audio: '#8b5cf6',
  archive: '#f59e0b',
  executable: '#ef4444',
  database: '#f97316',
  default: '#6b7280',
};

export function getCategoryColor(ext: string): string {
  const info = getFileTypeInfo('dummy.' + ext);
  return info.color;
}
