//! The resolution strategies the descriptors name.

use std::path::Path;

use super::{Descriptor, ManifestFile};
use crate::analysis::resolve::{Resolution, Unresolved};
use crate::analysis::workspace::{Workspace, dirname, join};

pub(super) fn resolve(
    ws: &Workspace,
    manifests: &[ManifestFile],
    id: &str,
    descriptor: &Descriptor,
    from: &str,
    specifier: &str,
) -> Resolution {
    let ctx = Ctx {
        ws,
        manifests,
        id,
        descriptor,
        from,
    };
    match descriptor.resolve.as_str() {
        "module-path" => ctx.module_path(specifier),
        "dotted-path" => ctx.dotted_path(specifier),
        "package-uri" => ctx.package_uri(specifier),
        "psr-4" => ctx.psr4(specifier),
        "snake-path" => ctx.snake_path(specifier),
        "namespace-prefix" => ctx.namespace_prefix(specifier),
        "require-path" => ctx.require_path(specifier),
        _ => Resolution::External,
    }
}

struct Ctx<'a> {
    ws: &'a Workspace,
    manifests: &'a [ManifestFile],
    id: &'a str,
    descriptor: &'a Descriptor,
    from: &'a str,
}

fn not_found() -> Resolution {
    Resolution::Unresolved(Unresolved::NotFound)
}

impl Ctx<'_> {
    fn own(&self) -> impl Iterator<Item = &ManifestFile> {
        self.manifests.iter().filter(|m| m.ecosystem == self.id)
    }

    /// Source roots: the repository's and each manifest's, joined with the descriptor's roots.
    fn roots(&self) -> Vec<String> {
        let mut bases = vec![String::new()];
        bases.extend(self.own().map(|m| m.dir.clone()));
        let mut roots = Vec::new();
        for base in bases {
            for root in &self.descriptor.roots {
                let dir = join(&base, root);
                if !roots.contains(&dir) {
                    roots.push(dir);
                }
            }
        }
        roots
    }

    fn source_in(&self, base: &str) -> Option<String> {
        self.descriptor
            .extensions
            .iter()
            .map(|e| format!("{base}.{e}"))
            .find(|p| self.ws.exists(p))
    }

    /// Go: a package is a directory; its module is the nearest `go.mod` name that prefixes the path.
    fn module_path(&self, spec: &str) -> Resolution {
        let module = self
            .own()
            .filter_map(|m| Some((m.name.as_deref()?, m)))
            .filter(|(name, _)| spec == *name || spec.starts_with(&format!("{name}/")))
            .max_by_key(|(name, _)| name.len());
        let Some((name, manifest)) = module else {
            return Resolution::External;
        };
        let dir = join(
            &manifest.dir,
            spec.strip_prefix(name)
                .unwrap_or("")
                .trim_start_matches('/'),
        );
        let file = self
            .ws
            .files_with(&["go"])
            .into_iter()
            .find(|f| dirname(f) == dir && !f.ends_with("_test.go"));
        file.map_or_else(not_found, |f| Resolution::File(f.to_owned()))
    }

    /// Python, Java, Kotlin: `a.b.C` is `a/b/C` under a source root. Leading dots climb from the
    /// importing file's directory.
    fn dotted_path(&self, spec: &str) -> Resolution {
        let dots = spec.len() - spec.trim_start_matches('.').len();
        let rest = spec.trim_start_matches('.');
        let segments: Vec<&str> = rest.split('.').filter(|s| !s.is_empty()).collect();
        if dots > 0 {
            let mut dir = dirname(self.from).to_owned();
            for _ in 1..dots {
                dir = dirname(&dir).to_owned();
            }
            let base = join(&dir, &segments.join("/"));
            return self
                .module_at(&base)
                .map_or_else(not_found, Resolution::File);
        }
        if self.descriptor.roots.is_empty() {
            return self.by_package_directory(&segments);
        }
        let roots = self.roots();
        for take in (1..=segments.len()).rev() {
            let path = segments[..take].join("/");
            for root in &roots {
                if let Some(file) = self.module_at(&join(root, &path)) {
                    return Resolution::File(file);
                }
            }
        }
        // The repository's own directories, but no file: the import is broken. Otherwise it names
        // a dependency or the platform.
        let probe = segments.len().saturating_sub(1).clamp(1, 3);
        let own = roots
            .iter()
            .any(|r| self.ws.has_dir(&join(r, &segments[..probe].join("/"))));
        if own && segments.len() > 1 {
            not_found()
        } else {
            Resolution::External
        }
    }

    /// Java and Kotlin: the directories of a file are its package, wherever the source root is.
    /// An import may also name a top-level function, a constant or a nested class, which no file is
    /// called after: then the package's own files stand for it. Never reported as broken.
    fn by_package_directory(&self, segments: &[&str]) -> Resolution {
        let extensions: Vec<&str> = self
            .descriptor
            .extensions
            .iter()
            .map(String::as_str)
            .collect();
        for take in (1..=segments.len()).rev() {
            let path = segments[..take].join("/");
            if let Some(file) = extensions
                .iter()
                .find_map(|e| self.ws.file_ending(&format!("{path}.{e}")))
            {
                return Resolution::File(file);
            }
        }
        for take in (1..segments.len()).rev() {
            if let Some(file) = self
                .ws
                .file_in_dir_ending(&segments[..take].join("/"), &extensions)
            {
                return Resolution::File(file);
            }
        }
        Resolution::External
    }

    /// A source file, or a Python package, at `base`.
    fn module_at(&self, base: &str) -> Option<String> {
        self.source_in(base).or_else(|| {
            let init = format!("{base}/__init__.py");
            (self.id == "python" && self.ws.exists(&init)).then_some(init)
        })
    }

    /// Dart: `package:name/path` is `lib/path` of the package called `name`.
    fn package_uri(&self, spec: &str) -> Resolution {
        if let Some(rest) = spec.strip_prefix("package:") {
            let (name, path) = rest.split_once('/').unwrap_or((rest, ""));
            return match self.own().find(|m| m.name.as_deref() == Some(name)) {
                Some(m) => {
                    let file = join(&join(&m.dir, "lib"), path);
                    if self.ws.exists(&file) {
                        Resolution::File(file)
                    } else {
                        not_found()
                    }
                }
                None => Resolution::External,
            };
        }
        if spec.contains(':') {
            return Resolution::External;
        }
        let file = join(dirname(self.from), spec);
        if self.ws.exists(&file) {
            Resolution::File(file)
        } else {
            not_found()
        }
    }

    /// PHP Composer: a namespace prefix maps to a directory (`autoload.psr-4`); a string is a path.
    fn psr4(&self, spec: &str) -> Resolution {
        if spec.contains('/')
            || Path::new(spec)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("php"))
        {
            let file = join(dirname(self.from), spec);
            return if self.ws.exists(&file) {
                Resolution::File(file)
            } else {
                not_found()
            };
        }
        let spec = spec.trim_start_matches('\\');
        let mut matching: Vec<(&ManifestFile, &str, &str)> = self
            .own()
            .flat_map(|m| {
                m.namespaces
                    .iter()
                    .map(move |(ns, dir)| (m, ns.as_str(), dir.as_str()))
            })
            .filter(|(_, ns, _)| spec.starts_with(ns.trim_start_matches('\\')))
            .collect();
        if matching.is_empty() {
            return Resolution::External;
        }
        matching.sort_by_key(|(_, ns, _)| std::cmp::Reverse(ns.len()));
        for (manifest, ns, dir) in matching {
            let rest = spec[ns.trim_start_matches('\\').len()..].replace('\\', "/");
            let mut parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
            while !parts.is_empty() {
                let file = join(
                    &join(&manifest.dir, dir),
                    &format!("{}.php", parts.join("/")),
                );
                if self.ws.exists(&file) {
                    return Resolution::File(file);
                }
                parts.pop();
            }
        }
        not_found()
    }

    /// Elixir: `MyApp.Foo.Bar` is `my_app/foo/bar.ex` under `lib`; a nested module may be defined in
    /// the file of its parent.
    fn snake_path(&self, spec: &str) -> Resolution {
        let segments: Vec<String> = spec.split('.').map(snake).collect();
        let roots = self.roots();
        for take in (1..=segments.len()).rev() {
            let path = segments[..take].join("/");
            for root in &roots {
                if let Some(file) = self.source_in(&join(root, &path)) {
                    return Resolution::File(file);
                }
            }
        }
        // A module may be defined in any file: no file by the convention is not a broken import.
        Resolution::External
    }

    /// C#: a namespace belongs to the project whose root namespace is its longest prefix.
    fn namespace_prefix(&self, spec: &str) -> Resolution {
        self.own()
            .filter_map(|m| Some((m.name.as_deref()?, m)))
            .filter(|(name, _)| spec == *name || spec.starts_with(&format!("{name}.")))
            .max_by_key(|(name, _)| name.len())
            .map_or(Resolution::External, |(_, m)| {
                Resolution::Package(m.dir.clone())
            })
    }

    /// Ruby: relative to the file, then under the load path.
    fn require_path(&self, spec: &str) -> Resolution {
        let name = spec.strip_suffix(".rb").unwrap_or(spec);
        let relative = join(dirname(self.from), name);
        if let Some(file) = self.source_in(&relative) {
            return Resolution::File(file);
        }
        for root in self.roots() {
            if let Some(file) = self.source_in(&join(&root, name)) {
                return Resolution::File(file);
            }
        }
        if spec.starts_with('.') {
            not_found()
        } else {
            Resolution::External
        }
    }
}

/// `MyApp` is `my_app`, `HTTPServer` is `http_server`.
fn snake(segment: &str) -> String {
    let chars: Vec<char> = segment.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let after_lower =
                i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let ends_acronym = i > 0
                && chars[i - 1].is_uppercase()
                && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if after_lower || ends_acronym {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(*c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::snake;

    #[test]
    fn module_names_become_file_names() {
        assert_eq!(snake("MyApp"), "my_app");
        assert_eq!(snake("HTTPServer"), "http_server");
        assert_eq!(snake("Repo"), "repo");
        assert_eq!(snake("V2Api"), "v2_api");
    }
}
