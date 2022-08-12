use super::base_table::BaseTableTrait;
use super::challenges_endpoints::AllChallenges;
use crate::fri_domain::FriDomain;
use itertools::Itertools;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use twenty_first::shared_math::mpolynomial::{Degree, MPolynomial};
use twenty_first::shared_math::traits::{Inverse, ModPowU32, PrimeField};
use twenty_first::shared_math::x_field_element::XFieldElement;
use twenty_first::timing_reporter::TimingReporter;

// Generic methods specifically for tables that have been extended

type XWord = XFieldElement;

pub trait ExtensionTable: BaseTableTrait<XWord> + Sync {
    /// get boundary constraints if they are set; otherwise compute them, set them, and return them
    fn get_boundary_constraints(&self) -> Vec<MPolynomial<XWord>> {
        if let Some(bc) = &self.to_base().boundary_constraints {
            bc.to_owned()
        } else {
            panic!("Do not have boundary constraints! {}", &self.name());
        }
    }

    /// get transition constraints if they are set; otherwise compute them, set them, and return them
    fn get_transition_constraints(&self) -> Vec<MPolynomial<XWord>> {
        if let Some(tc) = &self.to_base().transition_constraints {
            tc.to_owned()
        } else {
            panic!("Do not have transition constraints! {}", &self.name());
        }
    }

    fn get_consistency_constraints(&self) -> Vec<MPolynomial<XWord>> {
        if let Some(cc) = &self.to_base().consistency_constraints {
            cc.to_owned()
        } else {
            panic!("Do not have consistency constraints! {} ", &self.name());
        }
    }

    /// get terminal constraints if they are set; otherwise compute them, set them, and return them
    fn get_terminal_constraints(&self) -> Vec<MPolynomial<XWord>> {
        if let Some(tc) = &self.to_base().terminal_constraints {
            tc.to_owned()
        } else {
            panic!("Do not have terminal constraints! {}", &self.name());
        }
    }

    /// max_degree
    /// Compute the degree of the largest-degree quotient from all
    /// AIR constraints that apply to the table.
    /// TODO: cover other constraints beyond just transitions
    /// TODO: work with unset/general terminals
    fn max_degree(&self) -> Degree {
        let degree_bounds: Vec<Degree> = vec![self.interpolant_degree(); self.full_width() * 2];

        // 1. Insert dummy challenges
        // 2. Refactor so we can calculate max_degree without specifying challenges
        //    (and possibly without even calling get_transition_constraints).
        self.dynamic_transition_constraints(&AllChallenges::dummy())
            .iter()
            .map(|air| {
                let symbolic_degree_bound: Degree = air.symbolic_degree_bound(&degree_bounds);
                let padded_height: Degree = self.padded_height() as Degree;

                symbolic_degree_bound - padded_height + 1
            })
            .max()
            .unwrap_or(-1)
    }

    fn dynamic_transition_constraints(
        &self,
        challenges: &AllChallenges,
    ) -> Vec<MPolynomial<XFieldElement>>;

    fn all_quotient_degree_bounds(&self) -> Vec<Degree> {
        vec![
            self.boundary_quotient_degree_bounds(),
            self.transition_quotient_degree_bounds(),
            self.consistency_quotient_degree_bounds(),
            self.terminal_quotient_degree_bounds(),
        ]
        .concat()
    }

    fn boundary_quotient_degree_bounds(&self) -> Vec<Degree> {
        let max_degrees: Vec<Degree> = vec![self.interpolant_degree(); self.full_width()];

        let degree_bounds: Vec<Degree> = self
            .get_boundary_constraints()
            .iter()
            .map(|mpo| mpo.symbolic_degree_bound(&max_degrees) - 1)
            .collect();

        degree_bounds
    }

    fn transition_quotient_degree_bounds(&self) -> Vec<Degree> {
        let max_degrees: Vec<Degree> = vec![self.interpolant_degree(); 2 * self.full_width()];
        let transition_constraints = self.get_transition_constraints();
        // Safe because padded height is at most 2^30.
        let padded_height: Degree = self.padded_height().try_into().unwrap();
        transition_constraints
            .iter()
            .map(|mpo| mpo.symbolic_degree_bound(&max_degrees) - padded_height + 1)
            .collect()
    }

    fn consistency_quotient_degree_bounds(&self) -> Vec<Degree> {
        let max_degrees: Vec<Degree> = vec![self.interpolant_degree(); self.full_width()];
        self.get_consistency_constraints()
            .iter()
            .map(|mpo| mpo.symbolic_degree_bound(&max_degrees) - 1)
            .collect()
    }

    fn terminal_quotient_degree_bounds(&self) -> Vec<Degree> {
        let max_degrees: Vec<Degree> = vec![self.interpolant_degree(); self.full_width()];
        self.get_terminal_constraints()
            .iter()
            .map(|mpo| mpo.symbolic_degree_bound(&max_degrees) - 1)
            .collect::<Vec<Degree>>()
    }

    fn all_quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords: &[Vec<XWord>],
    ) -> Vec<Vec<XWord>> {
        let mut timer = TimingReporter::start();
        timer.elapsed(&format!("Table name: {}", self.name()));

        let boundary_quotients = self.boundary_quotients(fri_domain, codewords);
        timer.elapsed("boundary quotients");

        let transition_quotients = self.transition_quotients(fri_domain, codewords);
        timer.elapsed("transition quotients");

        let consistency_quotients = self.consistency_quotients(fri_domain, codewords);
        timer.elapsed("Done calculating consistency quotients");

        let terminal_quotients = self.terminal_quotients(fri_domain, codewords);
        timer.elapsed("terminal quotients");

        println!("{}", timer.finish());
        vec![
            boundary_quotients,
            transition_quotients,
            consistency_quotients,
            terminal_quotients,
        ]
        .concat()
    }

    fn transition_quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords_transposed: &[Vec<XWord>],
    ) -> Vec<Vec<XWord>> {
        let mut timer = TimingReporter::start();
        timer.elapsed("Start transition quotients");

        let one = XWord::ring_one();
        let x_values = fri_domain.domain_values();
        timer.elapsed("Domain values");

        let subgroup_zerofier: Vec<XWord> = x_values
            .iter()
            .map(|x| x.mod_pow_u32(self.padded_height() as u32) - one)
            .collect();
        let subgroup_zerofier_inverse = if self.padded_height() == 0 {
            subgroup_zerofier
        } else {
            XWord::batch_inversion(subgroup_zerofier)
        };
        timer.elapsed("Batch Inversion");
        let omicron_inverse = self.omicron().inverse();
        let zerofier_inverse: Vec<XWord> = x_values
            .into_iter()
            .enumerate()
            .map(|(i, x)| subgroup_zerofier_inverse[i] * (x - omicron_inverse))
            .collect();
        timer.elapsed("Zerofier Inverse");
        let transition_constraints = self.get_transition_constraints();
        timer.elapsed("Transition Constraints");

        let mut quotients: Vec<Vec<XWord>> = vec![];
        let unit_distance = self.unit_distance(fri_domain.length);

        for tc in transition_constraints.iter() {
            //timer.elapsed(&format!("START for-loop for tc of {}", tc.degree()));
            let quotient_codeword: Vec<XWord> = zerofier_inverse
                .par_iter()
                .enumerate()
                .map(|(i, z_inverse)| {
                    let current_row: Vec<XWord> = (0..self.full_width())
                        .map(|j| codewords_transposed[j][i])
                        .collect();

                    let next_row: Vec<XWord> = (0..self.full_width())
                        .map(|j| codewords_transposed[j][(i + unit_distance) % fri_domain.length])
                        .collect();

                    let point = vec![current_row, next_row].concat();
                    let composition_evaluation = tc.evaluate(&point);
                    composition_evaluation * *z_inverse
                })
                .collect();

            quotients.push(quotient_codeword);
            timer.elapsed(&format!("END for-loop for tc of {}", tc.degree()));
        }
        timer.elapsed("DONE Transition Constraints");

        self.debug_degree_bound_check(fri_domain, transition_constraints, &quotients);

        quotients
    }

    fn terminal_quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords: &[Vec<XWord>],
    ) -> Vec<Vec<XWord>> {
        for codeword in codewords.iter() {
            debug_assert_eq!(fri_domain.length, codeword.len());
        }

        // The zerofier for the terminal quotient has a root in the last
        // value in the cyclical group generated from omicron.
        let zerofier_codeword = fri_domain
            .domain_values()
            .into_iter()
            .map(|x| x - self.omicron().inverse())
            .collect_vec();

        let terminal_constraints = self.get_terminal_constraints();
        let quotient_codewords = self.quotients(
            fri_domain,
            codewords,
            zerofier_codeword,
            &terminal_constraints,
        );
        self.debug_degree_bound_check(fri_domain, terminal_constraints, &quotient_codewords);

        quotient_codewords
    }

    fn boundary_quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords: &[Vec<XWord>],
    ) -> Vec<Vec<XWord>> {
        for codeword in codewords.iter() {
            debug_assert_eq!(fri_domain.length, codeword.len());
        }

        let zerofier_codeword = fri_domain
            .domain_values()
            .into_iter()
            .map(|x| x - XFieldElement::ring_one())
            .collect();

        let boundary_constraints = self.get_boundary_constraints();
        let quotient_codewords = self.quotients(
            fri_domain,
            codewords,
            zerofier_codeword,
            &boundary_constraints,
        );
        self.debug_degree_bound_check(fri_domain, boundary_constraints, &quotient_codewords);

        quotient_codewords
    }

    fn consistency_quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords: &[Vec<XWord>],
    ) -> Vec<Vec<XWord>> {
        for codeword in codewords.iter() {
            debug_assert_eq!(fri_domain.length, codeword.len());
        }

        let zerofier_codeword = fri_domain
            .domain_values()
            .iter()
            .map(|x| x.mod_pow_u32(self.padded_height() as u32) - XWord::ring_one())
            .collect();

        let consistency_constraints = self.get_consistency_constraints();
        let quotient_codewords = self.quotients(
            fri_domain,
            codewords,
            zerofier_codeword,
            &consistency_constraints,
        );
        self.debug_degree_bound_check(fri_domain, consistency_constraints, &quotient_codewords);

        quotient_codewords
    }

    fn quotients(
        &self,
        fri_domain: &FriDomain<XWord>,
        codewords: &[Vec<XWord>],
        zerofier: Vec<XFieldElement>,
        constraints: &[MPolynomial<XWord>],
    ) -> Vec<Vec<XWord>> {
        let zerofier_inverse = if self.padded_height() == 0 {
            zerofier
        } else {
            XWord::batch_inversion(zerofier)
        };

        let mut quotient_codewords = vec![];
        for constraint in constraints.iter() {
            let quotient_codeword: Vec<_> = (0..fri_domain.length)
                .into_par_iter()
                .map(|fri_dom_i| {
                    let point: Vec<_> = (0..self.full_width())
                        .map(|j| codewords[j][fri_dom_i])
                        .collect();
                    constraint.evaluate(&point) * zerofier_inverse[fri_dom_i]
                })
                .collect();
            quotient_codewords.push(quotient_codeword);
        }
        quotient_codewords
    }

    fn debug_degree_bound_check(
        &self,
        fri_domain: &FriDomain<XWord>,
        consistency_constraints: Vec<MPolynomial<XWord>>,
        quotient_codewords: &Vec<Vec<XFieldElement>>,
    ) {
        if std::env::var("DEBUG").is_err() {
            return;
        }
        for (idx, qc) in quotient_codewords.iter().enumerate() {
            let interpolated = fri_domain.interpolate(qc);
            assert!(
                interpolated.degree() < fri_domain.length as isize - 1,
                "Degree of boundary quotient number {idx} (of {}) in {} must not be maximal. \
                    Got degree {}, and FRI domain length was {}.\
                    Unsatisfied constraint: {}",
                quotient_codewords.len(),
                self.name(),
                interpolated.degree(),
                fri_domain.length,
                consistency_constraints[idx]
            );
        }
    }
}
