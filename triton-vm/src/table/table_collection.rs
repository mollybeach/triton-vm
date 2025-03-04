use super::base_matrix::BaseMatrices;
use super::base_table::TableLike;
use super::challenges_endpoints::{AllChallenges, AllInitials, AllTerminals};
use super::extension_table::QuotientableExtensionTable;
use super::hash_table::{ExtHashTable, HashTable};
use super::instruction_table::{ExtInstructionTable, InstructionTable};
use super::jump_stack_table::{ExtJumpStackTable, JumpStackTable};
use super::op_stack_table::{ExtOpStackTable, OpStackTable};
use super::processor_table::{ExtProcessorTable, ProcessorTable};
use super::program_table::{ExtProgramTable, ProgramTable};
use super::ram_table::{ExtRamTable, RamTable};
use super::u32_op_table::{ExtU32OpTable, U32OpTable};
use crate::fri_domain::FriDomain;
use crate::stark::StarkHasher;
use crate::table::base_table::{Extendable, InheritsFromTable};
use crate::table::extension_table::DegreeWithOrigin;
use itertools::Itertools;
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::mpolynomial::Degree;
use twenty_first::shared_math::other::{is_power_of_two, roundup_npo2};
use twenty_first::shared_math::traits::FiniteField;
use twenty_first::shared_math::x_field_element::XFieldElement;
use twenty_first::timing_reporter::TimingReporter;

pub const NUM_TABLES: usize = 8;

#[derive(Debug, Clone)]
pub struct BaseTableCollection {
    /// The number of `data` rows after padding
    pub padded_height: usize,

    pub program_table: ProgramTable,
    pub instruction_table: InstructionTable,
    pub processor_table: ProcessorTable,
    pub op_stack_table: OpStackTable,
    pub ram_table: RamTable,
    pub jump_stack_table: JumpStackTable,
    pub hash_table: HashTable,
    pub u32_op_table: U32OpTable,
}

#[derive(Debug, Clone)]
pub struct ExtTableCollection {
    /// The number of `data` rows after padding
    pub padded_height: usize,

    pub program_table: ExtProgramTable,
    pub instruction_table: ExtInstructionTable,
    pub processor_table: ExtProcessorTable,
    pub op_stack_table: ExtOpStackTable,
    pub ram_table: ExtRamTable,
    pub jump_stack_table: ExtJumpStackTable,
    pub hash_table: ExtHashTable,
    pub u32_op_table: ExtU32OpTable,
}

/// A `TableId` uniquely determines one of Triton VM's tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableId {
    ProgramTable,
    InstructionTable,
    ProcessorTable,
    OpStackTable,
    RamTable,
    JumpStackTable,
    HashTable,
    U32OpTable,
}

/// Convert vector-of-arrays to vector-of-vectors.
fn to_vec_vecs<T: Sized + Clone, const S: usize>(vector_of_arrays: &[[T; S]]) -> Vec<Vec<T>> {
    vector_of_arrays
        .iter()
        .map(|arr| arr.to_vec())
        .collect_vec()
}

pub fn interpolant_degree(padded_height: usize, num_trace_randomizers: usize) -> Degree {
    (padded_height + num_trace_randomizers - 1) as Degree
}

pub fn derive_omicron<DataPF: FiniteField>(padded_height: u64) -> DataPF {
    debug_assert!(
        0 == padded_height || is_power_of_two(padded_height),
        "The padded height was: {}",
        padded_height
    );
    DataPF::primitive_root_of_unity(padded_height).unwrap()
}

impl BaseTableCollection {
    pub fn from_base_matrices(base_matrices: &BaseMatrices) -> Self {
        let padded_height = Self::padded_height(base_matrices);

        let program_table = ProgramTable::new_prover(to_vec_vecs(&base_matrices.program_matrix));
        let processor_table =
            ProcessorTable::new_prover(to_vec_vecs(&base_matrices.processor_matrix));
        let instruction_table =
            InstructionTable::new_prover(to_vec_vecs(&base_matrices.instruction_matrix));
        let op_stack_table = OpStackTable::new_prover(to_vec_vecs(&base_matrices.op_stack_matrix));
        let ram_table = RamTable::new_prover(to_vec_vecs(&base_matrices.ram_matrix));
        let jump_stack_table =
            JumpStackTable::new_prover(to_vec_vecs(&base_matrices.jump_stack_matrix));
        let hash_table = HashTable::new_prover(to_vec_vecs(&base_matrices.hash_matrix));
        let u32_op_table = U32OpTable::new_prover(to_vec_vecs(&base_matrices.u32_op_matrix));

        BaseTableCollection {
            padded_height,
            program_table,
            instruction_table,
            processor_table,
            op_stack_table,
            ram_table,
            jump_stack_table,
            hash_table,
            u32_op_table,
        }
    }

    pub fn padded_height(base_matrices: &BaseMatrices) -> usize {
        let max_height = [
            1, // minimum max height
            base_matrices.program_matrix.len(),
            base_matrices.processor_matrix.len(),
            base_matrices.instruction_matrix.len(),
            base_matrices.op_stack_matrix.len(),
            base_matrices.ram_matrix.len(),
            base_matrices.jump_stack_matrix.len(),
            base_matrices.hash_matrix.len(),
            base_matrices.u32_op_matrix.len(),
        ]
        .into_iter()
        .max()
        .unwrap();

        roundup_npo2(max_height as u64) as usize
    }

    pub fn codeword_tables(
        &self,
        fri_domain: &FriDomain<BFieldElement>,
        num_trace_randomizers: usize,
    ) -> BaseTableCollection {
        let padded_height = self.padded_height;
        let omicron = derive_omicron(padded_height as u64);

        let program_table = self.program_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let instruction_table = self.instruction_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let processor_table = self.processor_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let op_stack_table = self.op_stack_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let ram_table = self.ram_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let jump_stack_table = self.jump_stack_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let hash_table = self.hash_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );
        let u32_op_table = self.u32_op_table.codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
        );

        BaseTableCollection {
            padded_height,
            program_table,
            instruction_table,
            processor_table,
            op_stack_table,
            ram_table,
            jump_stack_table,
            hash_table,
            u32_op_table,
        }
    }

    pub fn get_all_base_columns(&self) -> Vec<Vec<BFieldElement>> {
        self.into_iter()
            .map(|table| table.data().clone())
            .collect_vec()
            .concat()
    }

    pub fn get_base_degree_bounds(&self, num_trace_randomizers: usize) -> Vec<Degree> {
        let sum_of_base_widths = self.into_iter().map(|table| table.base_width()).sum();
        vec![interpolant_degree(self.padded_height, num_trace_randomizers); sum_of_base_widths]
    }

    pub fn pad(&mut self) {
        let padded_height = self.padded_height;
        self.program_table.pad(padded_height);
        self.instruction_table.pad(padded_height);
        self.processor_table.pad(padded_height);
        self.op_stack_table.pad(padded_height);
        self.ram_table.pad(padded_height);
        self.jump_stack_table.pad(padded_height);
        self.hash_table.pad(padded_height);
        self.u32_op_table.pad(padded_height);
    }
}

impl<'a> IntoIterator for &'a BaseTableCollection {
    type Item = &'a dyn TableLike<BFieldElement>;

    type IntoIter = std::array::IntoIter<&'a dyn TableLike<BFieldElement>, NUM_TABLES>;

    fn into_iter(self) -> Self::IntoIter {
        [
            &self.program_table as &'a dyn TableLike<BFieldElement>,
            &self.instruction_table as &'a dyn TableLike<BFieldElement>,
            &self.processor_table as &'a dyn TableLike<BFieldElement>,
            &self.op_stack_table as &'a dyn TableLike<BFieldElement>,
            &self.ram_table as &'a dyn TableLike<BFieldElement>,
            &self.jump_stack_table as &'a dyn TableLike<BFieldElement>,
            &self.hash_table as &'a dyn TableLike<BFieldElement>,
            &self.u32_op_table as &'a dyn TableLike<BFieldElement>,
        ]
        .into_iter()
    }
}

impl ExtTableCollection {
    pub fn with_padded_height(padded_height: usize) -> Self {
        ExtTableCollection {
            padded_height,
            program_table: Default::default(),
            instruction_table: Default::default(),
            processor_table: Default::default(),
            op_stack_table: Default::default(),
            ram_table: Default::default(),
            jump_stack_table: Default::default(),
            hash_table: Default::default(),
            u32_op_table: Default::default(),
        }
    }

    pub fn for_verifier(
        num_trace_randomizers: usize,
        padded_height: usize,
        challenges: &AllChallenges,
        terminals: &AllTerminals<StarkHasher>,
    ) -> Self {
        let interpolant_degree = interpolant_degree(padded_height, num_trace_randomizers);

        let ext_program_table =
            ProgramTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_instruction_table =
            InstructionTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_processor_table =
            ProcessorTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_op_stack_table =
            OpStackTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_ram_table = RamTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_jump_stack_table =
            JumpStackTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_hash_table = HashTable::for_verifier(interpolant_degree, challenges, terminals);
        let ext_u32_op_table = U32OpTable::for_verifier(interpolant_degree, challenges, terminals);

        ExtTableCollection {
            padded_height,
            program_table: ext_program_table,
            instruction_table: ext_instruction_table,
            processor_table: ext_processor_table,
            op_stack_table: ext_op_stack_table,
            ram_table: ext_ram_table,
            jump_stack_table: ext_jump_stack_table,
            hash_table: ext_hash_table,
            u32_op_table: ext_u32_op_table,
        }
    }

    pub fn max_degree_with_origin(&self, num_trace_randomizers: usize) -> DegreeWithOrigin {
        self.into_iter()
            .map(|ext_table| {
                ext_table.all_degrees_with_origin(self.padded_height, num_trace_randomizers)
            })
            .concat()
            .into_iter()
            .max()
            .unwrap_or_default()
    }

    /// Create an ExtTableCollection from a BaseTableCollection by
    /// `.extend()`ing each base table.
    ///
    /// The `.extend()` for each table is specific to that table, but always
    /// involves adding some number of columns. Each table only needs their
    /// own challenges and initials, but `AllChallenges` and `AllInitials`
    /// are passed everywhere to keep each table's `.extend()` homogenous.
    pub fn extend_tables(
        base_tables: &BaseTableCollection,
        all_challenges: &AllChallenges,
        all_initials: &AllInitials<StarkHasher>,
        num_trace_randomizers: usize,
    ) -> (Self, AllTerminals<StarkHasher>) {
        let padded_height = base_tables.padded_height;
        let interpolant_degree = interpolant_degree(padded_height, num_trace_randomizers);

        let (program_table, program_table_terminals) = base_tables.program_table.extend(
            &all_challenges.program_table_challenges,
            &all_initials.program_table_endpoints,
            interpolant_degree,
        );

        let (instruction_table, instruction_table_terminals) =
            base_tables.instruction_table.extend(
                &all_challenges.instruction_table_challenges,
                &all_initials.instruction_table_endpoints,
                interpolant_degree,
            );

        let (processor_table, processor_table_terminals) = base_tables.processor_table.extend(
            &all_challenges.processor_table_challenges,
            &all_initials.processor_table_endpoints,
            interpolant_degree,
        );

        let (op_stack_table, op_stack_table_terminals) = base_tables.op_stack_table.extend(
            &all_challenges.op_stack_table_challenges,
            &all_initials.op_stack_table_endpoints,
            interpolant_degree,
        );

        let (ram_table, ram_table_terminals) = base_tables.ram_table.extend(
            &all_challenges.ram_table_challenges,
            &all_initials.ram_table_endpoints,
            interpolant_degree,
        );

        let (jump_stack_table, jump_stack_table_terminals) = base_tables.jump_stack_table.extend(
            &all_challenges.jump_stack_table_challenges,
            &all_initials.jump_stack_table_endpoints,
            interpolant_degree,
        );

        let (hash_table, hash_table_terminals) = base_tables.hash_table.extend(
            &all_challenges.hash_table_challenges,
            &all_initials.hash_table_endpoints,
            interpolant_degree,
        );

        let (u32_op_table, u32_op_table_terminals) = base_tables.u32_op_table.extend(
            &all_challenges.u32_op_table_challenges,
            &all_initials.u32_op_table_endpoints,
            interpolant_degree,
        );

        let ext_tables = ExtTableCollection {
            padded_height,
            program_table,
            instruction_table,
            processor_table,
            op_stack_table,
            ram_table,
            jump_stack_table,
            hash_table,
            u32_op_table,
        };

        let terminals = AllTerminals {
            program_table_endpoints: program_table_terminals,
            instruction_table_endpoints: instruction_table_terminals,
            processor_table_endpoints: processor_table_terminals,
            op_stack_table_endpoints: op_stack_table_terminals,
            ram_table_endpoints: ram_table_terminals,
            jump_stack_table_endpoints: jump_stack_table_terminals,
            hash_table_endpoints: hash_table_terminals,
            u32_op_table_endpoints: u32_op_table_terminals,
            phantom: std::marker::PhantomData,
        };

        (ext_tables, terminals)
    }

    pub fn codeword_tables(
        &self,
        fri_domain: &FriDomain<XFieldElement>,
        base_codeword_tables: BaseTableCollection,
        num_trace_randomizers: usize,
    ) -> Self {
        let padded_height = self.padded_height;
        let omicron = derive_omicron(padded_height as u64);

        let program_table = self.program_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.program_table.data(),
        );
        let instruction_table = self.instruction_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.instruction_table.data(),
        );
        let processor_table = self.processor_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.processor_table.data(),
        );
        let op_stack_table = self.op_stack_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.op_stack_table.data(),
        );
        let ram_table = self.ram_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.ram_table.data(),
        );
        let jump_stack_table = self.jump_stack_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.jump_stack_table.data(),
        );
        let hash_table = self.hash_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.hash_table.data(),
        );
        let u32_op_table = self.u32_op_table.ext_codeword_table(
            fri_domain,
            omicron,
            padded_height,
            num_trace_randomizers,
            base_codeword_tables.u32_op_table.data(),
        );

        ExtTableCollection {
            padded_height,
            program_table,
            instruction_table,
            processor_table,
            op_stack_table,
            ram_table,
            jump_stack_table,
            hash_table,
            u32_op_table,
        }
    }

    pub fn get_all_extension_columns(&self) -> Vec<Vec<XFieldElement>> {
        let mut all_ext_cols = vec![];

        for table in self.into_iter() {
            for col in table.data().iter().skip(table.base_width()) {
                all_ext_cols.push(col.clone());
            }
        }
        all_ext_cols
    }

    pub fn data(&self, table_id: TableId) -> &Vec<Vec<XFieldElement>> {
        use TableId::*;

        match table_id {
            ProgramTable => self.program_table.data(),
            InstructionTable => self.instruction_table.data(),
            ProcessorTable => self.processor_table.data(),
            OpStackTable => self.op_stack_table.data(),
            RamTable => self.ram_table.data(),
            JumpStackTable => self.jump_stack_table.data(),
            HashTable => self.hash_table.data(),
            U32OpTable => self.u32_op_table.data(),
        }
    }

    pub fn get_all_base_degree_bounds(&self, num_trace_randomizers: usize) -> Vec<Degree> {
        let sum_base_widths = self.into_iter().map(|table| table.base_width()).sum();
        vec![interpolant_degree(self.padded_height, num_trace_randomizers); sum_base_widths]
    }

    pub fn get_extension_degree_bounds(&self, num_trace_randomizers: usize) -> Vec<Degree> {
        let sum_base_widths: usize = self.into_iter().map(|table| table.base_width()).sum();
        let sum_full_widths: usize = self.into_iter().map(|table| table.full_width()).sum();
        let num_extension_columns = sum_full_widths - sum_base_widths;
        vec![interpolant_degree(self.padded_height, num_trace_randomizers); num_extension_columns]
    }

    pub fn get_all_quotients(
        &self,
        fri_domain: &FriDomain<XFieldElement>,
    ) -> Vec<Vec<XFieldElement>> {
        let mut timer = TimingReporter::start();
        let padded_height = self.padded_height;
        let omicron = derive_omicron(padded_height as u64);

        self.into_iter()
            .map(|ext_codeword_table| {
                timer.elapsed(&format!(
                    "Start calculating quotient: {}",
                    ext_codeword_table.name()
                ));
                let res = ext_codeword_table.all_quotients(
                    fri_domain,
                    ext_codeword_table.data(),
                    omicron,
                    padded_height,
                );
                timer.elapsed(&format!(
                    "Ended calculating quotient: {}",
                    ext_codeword_table.name()
                ));
                res
            })
            .concat()
    }

    pub fn get_all_quotient_degree_bounds(&self, num_trace_randomizers: usize) -> Vec<Degree> {
        self.into_iter() // Can we parallelize this? -> implement into_par_iter for TableCollection
            .map(|ext_table| {
                ext_table.get_all_quotient_degree_bounds(self.padded_height, num_trace_randomizers)
            })
            .concat()
    }
}

impl<'a> IntoIterator for &'a ExtTableCollection {
    type Item = &'a dyn QuotientableExtensionTable;

    type IntoIter = std::array::IntoIter<&'a dyn QuotientableExtensionTable, NUM_TABLES>;

    fn into_iter(self) -> Self::IntoIter {
        [
            &self.program_table as &'a dyn QuotientableExtensionTable,
            &self.instruction_table as &'a dyn QuotientableExtensionTable,
            &self.processor_table as &'a dyn QuotientableExtensionTable,
            &self.op_stack_table as &'a dyn QuotientableExtensionTable,
            &self.ram_table as &'a dyn QuotientableExtensionTable,
            &self.jump_stack_table as &'a dyn QuotientableExtensionTable,
            &self.hash_table as &'a dyn QuotientableExtensionTable,
            &self.u32_op_table as &'a dyn QuotientableExtensionTable,
        ]
        .into_iter()
    }
}

#[cfg(test)]
mod table_collection_tests {
    use super::*;
    use crate::table::{
        hash_table, instruction_table, jump_stack_table, op_stack_table, processor_table,
        program_table, ram_table, u32_op_table,
    };

    fn dummy_ext_table_collection() -> ExtTableCollection {
        let max_padded_height = 1;
        ExtTableCollection::with_padded_height(max_padded_height)
    }

    #[test]
    fn base_table_width_is_correct() {
        let base_matrices = BaseMatrices::default();
        let base_tables = BaseTableCollection::from_base_matrices(&base_matrices);

        assert_eq!(
            program_table::BASE_WIDTH,
            base_tables.program_table.base_width()
        );
        assert_eq!(
            instruction_table::BASE_WIDTH,
            base_tables.instruction_table.base_width()
        );
        assert_eq!(
            processor_table::BASE_WIDTH,
            base_tables.processor_table.base_width()
        );
        assert_eq!(
            op_stack_table::BASE_WIDTH,
            base_tables.op_stack_table.base_width()
        );
        assert_eq!(ram_table::BASE_WIDTH, base_tables.ram_table.base_width());
        assert_eq!(
            jump_stack_table::BASE_WIDTH,
            base_tables.jump_stack_table.base_width()
        );
        assert_eq!(hash_table::BASE_WIDTH, base_tables.hash_table.base_width());
        assert_eq!(
            u32_op_table::BASE_WIDTH,
            base_tables.u32_op_table.base_width()
        );
    }

    #[test]
    fn ext_table_width_is_correct() {
        let ext_tables = dummy_ext_table_collection();

        assert_eq!(
            program_table::FULL_WIDTH,
            ext_tables.program_table.full_width()
        );
        assert_eq!(
            instruction_table::FULL_WIDTH,
            ext_tables.instruction_table.full_width()
        );
        assert_eq!(
            processor_table::FULL_WIDTH,
            ext_tables.processor_table.full_width()
        );
        assert_eq!(
            op_stack_table::FULL_WIDTH,
            ext_tables.op_stack_table.full_width()
        );
        assert_eq!(ram_table::FULL_WIDTH, ext_tables.ram_table.full_width());
        assert_eq!(
            jump_stack_table::FULL_WIDTH,
            ext_tables.jump_stack_table.full_width()
        );
        assert_eq!(hash_table::FULL_WIDTH, ext_tables.hash_table.full_width());
        assert_eq!(
            u32_op_table::FULL_WIDTH,
            ext_tables.u32_op_table.full_width()
        );
    }
}
