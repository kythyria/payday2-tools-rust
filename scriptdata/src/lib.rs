mod document;

pub trait ScriptdataWriter {
    type Error;
    type Document;
    
    fn key<'s, K: Into<document::Key<'s>>>(&mut self, key: K) -> Result<(), Self::Error>;
    fn value<I: Into<document::ScalarItem>>(&mut self, value: I)-> Result<(), Self::Error>;
    
    fn begin_table(&mut self, meta: Option<&str>) -> Result<document::TableId, Self::Error>;
    fn end_table(&mut self)-> Result<(), Self::Error>;

    fn finish(self) -> Result<Self::Document, Self::Error>;
}

pub trait ReopeningWriter: ScriptdataWriter {
    fn reopen_table(&mut self, tid: document::TableId);
}