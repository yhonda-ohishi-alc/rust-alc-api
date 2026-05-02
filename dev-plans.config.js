export default {
  scopeLabels: ["rust-alc-api", "cross-repo"],
  grepPatterns: [/\bif_flag!\(\s*"([a-z][a-z0-9_]+)#([a-f0-9]{8})"/g],
  sourceDirs: ["src", "crates"],
};
