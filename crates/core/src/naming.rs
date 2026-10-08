//! Output filename resolution for batch cleaning (design §6.4).

use std::collections::HashSet;

/// Splits a filename into its stem and extension based on the last period.
///
/// For multi-part extensions like `archive.tar.gz`, only the final extension is split off:
/// stem is `archive.tar`, extension is `gz`.
/// If there is no period, stem is `filename` and extension is empty.
#[must_use]
pub fn split_stem_and_ext(filename: &str) -> (&str, &str) {
    match filename.rfind('.') {
        Some(idx) => (&filename[..idx], &filename[idx + 1..]),
        None => (filename, ""),
    }
}

/// Builds a numbered filename candidate following the `stem (n).ext` scheme (design §6.4).
///
/// If `n` is 0, `filename` is returned unchanged.
/// If `n` is 1 or greater, ` (n)` is inserted before the final extension,
/// or appended to the stem if there is no extension.
/// The original casing of both stem and extension is preserved.
#[must_use]
pub fn numbered_name(filename: &str, n: usize) -> String {
    if n == 0 {
        return filename.to_string();
    }
    let (stem, ext) = split_stem_and_ext(filename);
    if ext.is_empty() {
        format!("{stem} ({n})")
    } else {
        format!("{stem} ({n}).{ext}")
    }
}

/// Allocates the next available filename using the `name (n).ext` scheme (design §6.4).
///
/// Collision checks against `used_lower` are case-insensitive.
/// When an available name is found, its lowercase form is inserted into `used_lower`,
/// and the candidate (preserving original casing) is returned.
fn allocate_unique_name(filename: &str, used_lower: &mut HashSet<String>) -> String {
    let mut n = 0usize;
    loop {
        let candidate = numbered_name(filename, n);
        let candidate_lower = candidate.to_lowercase();
        if !used_lower.contains(&candidate_lower) {
            used_lower.insert(candidate_lower);
            return candidate;
        }
        n += 1;
    }
}

/// Resolves unique output filenames for batch cleaning (design §6.4).
///
/// Each output filename corresponds to the input filename at the same index in `inputs`.
/// The candidate name is the input filename itself.
/// Names are assigned in Unicode code point order of the input filenames so that results
/// are invariant to the input order.
/// If a candidate name collides with an existing file in `existing` or an already assigned
/// output name, an incrementing numeric suffix is appended: `name (1).ext`, `name (2).ext`, etc.
/// Collision checks are case-insensitive, while returned output names preserve the original
/// casing of the input filename.
pub fn resolve_output_names(inputs: &[String], existing: &HashSet<String>) -> Vec<String> {
    // Pair each input with its original index so results can be restored to input order.
    let mut indexed_inputs: Vec<(&str, usize)> = inputs
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // Sort by input filename in Unicode code point order (UTF-8 byte comparison).
    // Stable sort ensures predictable assignment order even if identical inputs are passed.
    indexed_inputs.sort_by(|a, b| a.0.cmp(b.0));

    // Track all lowercased names that are already occupied (existing files and assigned outputs).
    let mut used_lower: HashSet<String> = existing.iter().map(|s| s.to_lowercase()).collect();
    let mut resolved = vec![String::new(); inputs.len()];

    for (input, original_index) in indexed_inputs {
        resolved[original_index] = allocate_unique_name(input, &mut used_lower);
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_stem_and_ext_cases() {
        assert_eq!(split_stem_and_ext("photo.jpg"), ("photo", "jpg"));
        assert_eq!(split_stem_and_ext("archive.tar.gz"), ("archive.tar", "gz"));
        assert_eq!(split_stem_and_ext("README"), ("README", ""));
        assert_eq!(split_stem_and_ext(".gitignore"), ("", "gitignore"));
        assert_eq!(split_stem_and_ext("trailing."), ("trailing", ""));
        assert_eq!(split_stem_and_ext(""), ("", ""));
    }

    #[test]
    fn numbered_name_zero_and_positive() {
        assert_eq!(numbered_name("photo.jpg", 0), "photo.jpg");
        assert_eq!(numbered_name("photo.jpg", 1), "photo (1).jpg");
        assert_eq!(numbered_name("photo.jpg", 10), "photo (10).jpg");
        assert_eq!(numbered_name("README", 0), "README");
        assert_eq!(numbered_name("README", 1), "README (1)");
        assert_eq!(numbered_name("README", 2), "README (2)");
        assert_eq!(numbered_name("a.tar.gz", 0), "a.tar.gz");
        assert_eq!(numbered_name("a.tar.gz", 1), "a.tar (1).gz");
    }

    #[test]
    fn no_collision() {
        let inputs = vec!["doc1.png".to_string(), "doc2.jpg".to_string()];
        let existing = HashSet::new();
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["doc1.png", "doc2.jpg"]);
    }

    #[test]
    fn collision_with_existing_files() {
        let inputs = vec!["doc.png".to_string()];
        let mut existing = HashSet::new();
        existing.insert("doc.png".to_string());
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["doc (1).png"]);
    }

    #[test]
    fn collision_between_inputs() {
        // Two inputs with the identical filename (e.g. from different folders).
        let inputs = vec!["doc.png".to_string(), "doc.png".to_string()];
        let existing = HashSet::new();
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["doc.png", "doc (1).png"]);
    }

    #[test]
    fn numeric_suffix_rollover() {
        let inputs = vec!["doc.png".to_string()];
        let mut existing = HashSet::new();
        existing.insert("doc.png".to_string());
        for i in 1..=9 {
            existing.insert(format!("doc ({i}).png"));
        }
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["doc (10).png"]);

        // Rollover with intermediate numbers occupied:
        let inputs_multi = vec!["photo.jpg".to_string(), "image.png".to_string()];
        let mut existing_multi = HashSet::new();
        existing_multi.insert("photo.jpg".to_string());
        existing_multi.insert("photo (1).jpg".to_string());
        let result_multi = resolve_output_names(&inputs_multi, &existing_multi);
        assert_eq!(result_multi, vec!["photo (2).jpg", "image.png"]);
    }

    #[test]
    fn case_insensitivity() {
        // Collision check ignores case, returning candidate with original casing.
        let inputs = vec!["photo.jpg".to_string()];
        let mut existing = HashSet::new();
        existing.insert("PHOTO.JPG".to_string());
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["photo (1).jpg"]);

        // Input casing collision:
        // In Unicode code point order, 'A' (0x41) < 'a' (0x61), so "A.jpg" comes first.
        // "A.jpg" gets "A.jpg", and "a.jpg" collides case-insensitively, getting "a (1).jpg".
        let inputs_case = vec!["a.jpg".to_string(), "A.jpg".to_string()];
        let existing_empty = HashSet::new();
        let result_case = resolve_output_names(&inputs_case, &existing_empty);
        assert_eq!(result_case, vec!["a (1).jpg", "A.jpg"]);

        let inputs_case_rev = vec!["A.jpg".to_string(), "a.jpg".to_string()];
        let result_case_rev = resolve_output_names(&inputs_case_rev, &existing_empty);
        assert_eq!(result_case_rev, vec!["A.jpg", "a (1).jpg"]);
    }

    #[test]
    fn multi_part_extension() {
        let inputs = vec!["a.tar.gz".to_string()];
        let mut existing = HashSet::new();
        existing.insert("a.tar.gz".to_string());
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["a.tar (1).gz"]);
    }

    #[test]
    fn no_extension() {
        let inputs = vec!["README".to_string()];
        let mut existing = HashSet::new();
        existing.insert("README".to_string());
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["README (1)"]);
    }

    #[test]
    fn invariant_under_input_order() {
        let mut existing = HashSet::new();
        existing.insert("alpha.png".to_string());

        let inputs_a = vec!["beta.png".to_string(), "alpha.png".to_string()];
        let result_a = resolve_output_names(&inputs_a, &existing);
        // beta.png is at idx 0, alpha.png is at idx 1
        assert_eq!(result_a, vec!["beta.png", "alpha (1).png"]);

        let inputs_b = vec!["alpha.png".to_string(), "beta.png".to_string()];
        let result_b = resolve_output_names(&inputs_b, &existing);
        // alpha.png is at idx 0, beta.png is at idx 1
        assert_eq!(result_b, vec!["alpha (1).png", "beta.png"]);

        // Input collisions invariance across casing
        let inputs_1 = vec!["doc.png".to_string(), "DOC.png".to_string()];
        let result_1 = resolve_output_names(&inputs_1, &HashSet::new());
        assert_eq!(result_1, vec!["doc (1).png", "DOC.png"]);

        let inputs_2 = vec!["DOC.png".to_string(), "doc.png".to_string()];
        let result_2 = resolve_output_names(&inputs_2, &HashSet::new());
        assert_eq!(result_2, vec!["DOC.png", "doc (1).png"]);
    }

    #[test]
    fn casing_preservation_of_extension() {
        let inputs = vec!["IMG.JPG".to_string()];
        let mut existing = HashSet::new();
        existing.insert("IMG.JPG".to_string());
        let result = resolve_output_names(&inputs, &existing);
        assert_eq!(result, vec!["IMG (1).JPG"]);

        let inputs_mixed = vec!["photo.JpEg".to_string()];
        let mut existing_mixed = HashSet::new();
        existing_mixed.insert("photo.jpeg".to_string());
        let result_mixed = resolve_output_names(&inputs_mixed, &existing_mixed);
        assert_eq!(result_mixed, vec!["photo (1).JpEg"]);
    }
}
