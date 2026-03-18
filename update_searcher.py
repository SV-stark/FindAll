import re

with open("src/indexer/searcher.rs", "r", encoding="utf-8") as f:
    content = f.read()

new_search_sync = """
    pub fn search_sync(
        &self,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
        file_extensions: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        use super::query_parser::{extract_highlight_terms, ParsedQuery};

        // Create cache key
        let cache_key = CacheKey {
            query: query.to_string(),
            limit,
            min_size,
            max_size,
            extensions: file_extensions.map(|e| e.to_vec()),
        };

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let parsed = ParsedQuery::new(query);
        let highlight_terms = extract_highlight_terms(query);

        let searcher = self.reader.searcher();

        // Helper to run query with all filters
        let run_query = |text_query: Box<dyn tantivy::query::Query>| -> Result<(Box<dyn tantivy::query::Query>, Vec<(f32, tantivy::DocAddress)>)> {
            let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                vec![(Occur::Must, text_query)];

            if min_size.is_some() || max_size.is_some() {
                let lower = Term::from_field_u64(self.size_field, min_size.unwrap_or(0));
                let upper = Term::from_field_u64(self.size_field, max_size.unwrap_or(u64::MAX));
                let range = RangeQuery::new(Bound::Included(lower), Bound::Included(upper));
                combine.push((Occur::Must, Box::new(range)));
            }

            if let Some(extensions) = file_extensions {
                if !extensions.is_empty() {
                    let extension_queries: Vec<_> = extensions
                        .iter()
                        .map(|ext| {
                            let ext_lower = ext.to_lowercase();
                            let term = tantivy::Term::from_field_text(self.extension_field, &ext_lower);
                            tantivy::query::TermQuery::new(term, IndexRecordOption::Basic)
                        })
                        .collect();

                    if !extension_queries.is_empty() {
                        let extension_bool_query = tantivy::query::BooleanQuery::new(
                            extension_queries
                                .into_iter()
                                .map(|q| (Occur::Should, Box::new(q) as Box<dyn tantivy::query::Query>))
                                .collect(),
                        );
                        combine.push((Occur::Must, Box::new(extension_bool_query)));
                    }
                }
            }

            let final_query: Box<dyn tantivy::query::Query> = if combine.len() == 1 {
                combine.remove(0).1
            } else {
                Box::new(BooleanQuery::new(combine))
            };

            let top_docs = searcher
                .search(&*final_query, &TopDocs::with_limit(limit))
                .map_err(|e| FlashError::search(query, e.to_string()))?;
                
            Ok((final_query, top_docs))
        };

        // TIER 1: Exact search
        let exact_query = Box::new(
            self.query_parser
                .parse_query(&parsed.text_query)
                .unwrap_or_else(|_| tantivy::query::AllQuery.clone().into()),
        );
        let (mut final_query, mut top_docs) = run_query(exact_query)?;

        // TIER 2: Fuzzy Fallback
        static PHRASE_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let phrase_regex = PHRASE_REGEX.get_or_init(|| regex::Regex::new(r#""([^"]+)""#).unwrap());

        if top_docs.len() < limit && !phrase_regex.is_match(&parsed.text_query) && !parsed.text_query.trim().is_empty() {
            let fuzzy_query = self.build_fuzzy_query(&parsed.text_query)?;
            let (fuzzy_final_query, fuzzy_docs) = run_query(fuzzy_query)?;
            
            // Prefer highlighting based on the more permissive fuzzy query
            final_query = fuzzy_final_query;
            
            let mut seen: std::collections::HashSet<tantivy::DocAddress> = top_docs.iter().map(|(_, addr)| *addr).collect();
            for (score, doc_addr) in fuzzy_docs {
                if !seen.contains(&doc_addr) {
                    top_docs.push((score, doc_addr));
                    seen.insert(doc_addr);
                    if top_docs.len() >= limit {
                        break;
                    }
                }
            }
            top_docs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        let mut results = Vec::with_capacity(top_docs.len().min(limit));

        // Create snippet generator once outside the loop
        let snippet_generator =
            SnippetGenerator::create(&searcher, &*final_query, self.content_field)?;

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address).map_err(|e| {
                FlashError::search(query, format!("Failed to retrieve document: {}", e))
            })?;

            let file_path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string())
                .unwrap_or_default();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string());

            let extension = retrieved_doc
                .get_first(self.extension_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string());

            // Get fast fields for size and modified
            let size = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .u64("size")
                .ok()
                .map(|f| f.values.get_val(doc_address.doc_id));

            let modified = searcher
                .segment_reader(doc_address.segment_ord)
                .fast_fields()
                .date("modified")
                .ok()
                .map(|f| {
                    let date = f.values.get_val(doc_address.doc_id);
                    date.into_timestamp_secs() as u64
                });

            // Generates snippets and replaces HTML tags. 
            // NOTE: Offloading full rendering to UI.
            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
            let snippet_text = snippet.to_html()
                .replace("<b>", "")
                .replace("</b>", "")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&");

            let snippets = if snippet_text.is_empty() { vec![] } else { vec![snippet_text] };

            results.push(SearchResult {
                file_path,
                title,
                score,
                modified,
                size,
                extension,
                matched_terms: highlight_terms.clone(),
                snippets,
            });

            if results.len() >= limit {
                break;
            }
        }

        // Cache the results
        self.cache.insert(cache_key, results.clone());

        Ok(results)
    }
"""

start_idx = content.find("    pub fn search_sync(")
end_idx = content.find("    /// Build a fuzzy query", start_idx)

if start_idx != -1 and end_idx != -1:
    new_content = content[:start_idx] + new_search_sync.strip() + "\n\n" + content[end_idx:]
    with open("src/indexer/searcher.rs", "w", encoding="utf-8") as f:
        f.write(new_content)
    print("Successfully replaced search_sync")
else:
    print("Could not find start or end index")
