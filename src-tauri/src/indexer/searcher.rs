use crate::error::{FlashError, Result};
use serde::Serialize;
use std::ops::Bound;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery};
use tantivy::schema::{Field, Schema, Term, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

#[derive(Serialize, Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<String>,
    pub score: f32,
}

/// Manages searching the Tantivy index
pub struct IndexSearcher {
    reader: IndexReader,
    query_parser: QueryParser,
    schema: Schema,
    path_field: Field,
    title_field: Field,
    size_field: Field,
}

impl IndexSearcher {
    pub fn new(index: &Index) -> Result<Self> {
        let schema = index.schema();

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| FlashError::Index(e.to_string()))?;

        // Get field references once to avoid repeated lookups
        let path_field = schema
            .get_field("file_path")
            .map_err(|_| FlashError::Index("file_path field not found".to_string()))?;
        let title_field = schema
            .get_field("title")
            .map_err(|_| FlashError::Index("title field not found".to_string()))?;
        let size_field = schema
            .get_field("size")
            .map_err(|_| FlashError::Index("size field not found".to_string()))?;

        // Search across content, title, and file_path fields
        let default_fields: Vec<Field> = vec!["content", "title", "file_path"]
            .iter()
            .filter_map(|field_name| schema.get_field(field_name).ok())
            .collect();

        let query_parser = QueryParser::for_index(index, default_fields);

        Ok(Self {
            reader,
            query_parser,
            schema,
            path_field,
            title_field,
            size_field,
        })
    }

    /// Search the index and return top results with optional filters
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        min_size: Option<u64>,
        max_size: Option<u64>,
    ) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let text_query = self
            .query_parser
            .parse_query(query)
            .map_err(|e| FlashError::Search(e.to_string()))?;

        // Build query with optional filters
        let final_query: Box<dyn tantivy::query::Query> = match (min_size, max_size) {
            (None, None) => text_query,
            (min, max) => {
                let mut combine: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                    vec![(Occur::Must, text_query)];

                // Get the field name for the range query
                let size_field_name = self.schema.get_field_name(self.size_field).to_string();
                let value_type = tantivy::schema::Type::U64;

                if let Some(min_val) = min {
                    let lower = Term::from_field_u64(self.size_field, min_val);
                    let upper = Term::from_field_u64(self.size_field, u64::MAX);
                    let range = RangeQuery::new_term_bounds(
                        size_field_name.clone(),
                        value_type,
                        &Bound::Included(lower),
                        &Bound::Included(upper),
                    );
                    combine.push((Occur::Must, Box::new(range)));
                }

                if let Some(max_val) = max {
                    let lower = Term::from_field_u64(self.size_field, 0);
                    let upper = Term::from_field_u64(self.size_field, max_val);
                    let range = RangeQuery::new_term_bounds(
                        size_field_name.clone(),
                        value_type,
                        &Bound::Included(lower),
                        &Bound::Included(upper),
                    );
                    combine.push((Occur::Must, Box::new(range)));
                }

                Box::new(BooleanQuery::new(combine))
            }
        };

        let top_docs = searcher
            .search(&*final_query, &TopDocs::with_limit(limit))
            .map_err(|e| FlashError::Search(e.to_string()))?;

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| FlashError::Search(e.to_string()))?;

            let file_path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string())
                .unwrap_or_default();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|f| f.as_str())
                .map(|s: &str| s.to_string());

            results.push(SearchResult {
                file_path,
                title,
                score,
            });
        }

        Ok(results)
    }
}
