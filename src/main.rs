use std::cmp::Ordering::{Greater, Less};
use std::io;
use std::io::BufRead;

#[derive(Clone, Debug)]
struct Cidr {
    pub bits: Vec<bool>,
}

impl Cidr {
    fn get_bits(ipv4cidr: &str, size: usize) -> Vec<bool> {
        ipv4cidr
            .split('.')
            .flat_map(|group| {
                let g: i16 = group.parse().unwrap();
                [7, 6, 5, 4, 3, 2, 1, 0]
                    .iter()
                    .map(|b| g & (1 << b) > 0)
                    .collect::<Vec<bool>>()
            })
            .take(size)
            .collect()
    }
    fn bits(&self) -> &Vec<bool> {
        &self.bits
    }
    fn size(&self) -> usize {
        self.bits.len()
    }
    fn parse(s: &str) -> Self {
        let mut x = s.split('/');
        let ipv4cidr = x.next().unwrap();
        let size = x.next().unwrap().parse().unwrap();
        Cidr {
            bits: Self::get_bits(ipv4cidr, size),
        }
    }
    fn push(&self, b: bool) -> Self {
        let mut new = self.clone();
        new.bits.push(b);
        new
    }
    fn pop(&self) -> Self {
        let mut new = self.clone();
        new.bits.pop();
        new
    }
    fn to_pretty_string(&self) -> String {
        let mut groups = vec![0, 0, 0, 0];
        let bits = self.bits();
        for (i, x) in bits.iter().enumerate() {
            if *x {
                groups[i / 8] |= 1 << (7 - (i & 7));
            }
        }
        format!(
            "{}.{}.{}.{}/{}",
            groups[0],
            groups[1],
            groups[2],
            groups[3],
            self.bits.len()
        )
    }
}

#[derive(Debug)]
struct Tree {
    present: bool,
    pub node_count: usize,
    pub cidr_count: usize,
    pub coverage: f64,
    pub cidr: Cidr,
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
    pub best_coverage: Option<(f64, usize, Cidr)>,
}

impl Tree {
    fn new_node(cidr: Cidr) -> Self {
        Tree {
            cidr,
            present: false,
            cidr_count: 0,
            node_count: 1,
            coverage: 0.0,
            left: None,
            right: None,
            best_coverage: None,
        }
    }

    fn new() -> Self {
        Tree::new_node(Cidr::parse("0.0.0.0/0"))
    }

    fn make_present(&mut self) {
        self.present = true;

        // Remove any children as this new CIDR has full coverage anyway
        self.left = None;
        self.right = None;
    }

    fn optimize(&mut self) {
        let all_childs_present = [self.left.as_ref(), self.right.as_ref()]
            .iter()
            .all(|o| o.map(|t| t.present).unwrap_or(false));

        if all_childs_present {
            // Replace childs
            self.make_present()
        }
    }

    fn update_coverage(&mut self) {
        let childs = [self.left.as_ref(), self.right.as_ref()]
            .iter()
            .flatten()
            .map(|t| t.coverage)
            .sum::<f64>();
        self.coverage = if self.present { 1.0 } else { childs / 2.0 };
    }

    fn update_node_count(&mut self) {
        let childs = [self.left.as_ref(), self.right.as_ref()]
            .iter()
            .flatten()
            .map(|t| t.node_count)
            .sum::<usize>();
        self.node_count = 1 + childs;
    }

    fn update_cidr_count(&mut self) {
        let childs = [self.left.as_ref(), self.right.as_ref()]
            .iter()
            .flatten()
            .map(|t| t.cidr_count)
            .sum();
        self.cidr_count = if self.present { 1 } else { childs };
    }

    fn update_best_coverage(&mut self) {
        if self.present {
            self.best_coverage = None;
            return;
        }

        let me = Some((self.coverage(), self.cidr_count, self.cidr.clone()));
        let left = self.left.as_ref().and_then(|t| t.best_coverage.as_ref());
        let right = self.right.as_ref().and_then(|t| t.best_coverage.as_ref());
        let all = [me.as_ref(), left, right];

        let candidates = all
            .iter()
            .flatten()
            .cloned();

        let score = |s: i32, f: f64| 2.0_f64.powi(32 - s) * (1.0 - f);
        self.best_coverage = candidates
            .min_by(|a, b| {
                if score(a.2.size() as i32, a.0) < score(b.2.size() as i32, b.0) {
                    Less
                } else {
                    Greater
                }
            })
            .cloned()
    }

    fn insert_bits(&mut self, bits: &[bool]) {
        if self.present {
            return;
        }

        if let Some(bit) = bits.first().cloned() {
            // Get or create the child we need to go to
            let next = self.cidr.push(bit);
            let opt_child = if bit { &mut self.right } else { &mut self.left };
            let child = opt_child.get_or_insert_with(|| Box::new(Tree::new_node(next)));
            // Recursive insert
            child.insert_bits(&bits[1..]);
        } else {
            // We traversed the full path so this node is the one we want
            self.make_present()
        }

        self.optimize();
        self.update_cidr_count();
        self.update_node_count();
        self.update_coverage();
        self.update_best_coverage();
    }

    fn nodes(&self) -> usize {
        self.node_count
    }

    fn cidrs(&self) -> usize {
        self.cidr_count
    }

    fn insert(&mut self, cidr: &Cidr) {
        let bits = cidr.bits();
        self.insert_bits(&bits.as_slice()[..cidr.size()]);
    }

    fn coverage(&self) -> f64 {
        self.coverage
    }

    fn best_coverage(&self) -> Option<&(f64, usize, Cidr)> {
        self.best_coverage.as_ref()
    }

    fn print(&self) {
        if self.present {
            println!("{}", self.cidr.to_pretty_string());
        }

        [self.left.as_ref(), self.right.as_ref()]
            .iter()
            .flatten()
            .for_each(|t| t.print());
    }

    fn print_tree(&self, indent: String) {
        if self.present {
            println!("{} {}", indent, self.cidr.to_pretty_string());
        }

        [("0", self.left.as_ref()), ("1", self.right.as_ref())]
            .iter()
            .map(|(d, o)| o.map(|t| (d, t)))
            .flatten()
            .for_each(|(d, t)| t.print_tree(indent.clone() + d));
    }
}

fn main() {
    let mut tree = Tree::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        if let Ok(s) = line {
            let cidr = Cidr::parse(&s);
            tree.insert(&cidr);
        }
    }

    while tree.cidrs() > 40 {
        let best = tree.best_coverage().cloned();
        println!("coverage: {}, cidrs: {}", tree.coverage(), tree.cidrs());

        if let Some(pair) = best {
            tree.insert(&pair.2)
        }
    }

    println!("coverage: {}", tree.coverage());
    println!("nodes: {}", tree.nodes());
    println!("cidrs: {}", tree.cidrs());

    // tree.print();
    tree.print_tree("".to_string());
}

#[cfg(test)]
mod tests {
    use super::{Cidr, Tree};

    fn bits(s: &str) -> Vec<bool> {
        s.chars()
            .map(|c| if c == '0' { false } else { true })
            .collect()
    }

    #[test]
    fn check_cidr_to_bits() {
        assert_eq!(
            Cidr::parse("0.0.0.0/32").bits(),
            &bits("00000000000000000000000000000000")
        );
        assert_eq!(
            Cidr::parse("255.0.255.0/32").bits(),
            &bits("11111111000000001111111100000000")
        );
        assert_eq!(
            Cidr::parse("1.1.1.1/32").bits(),
            &bits("00000001000000010000000100000001")
        );
        assert_eq!(
            Cidr::parse("1.2.3.4/32").bits(),
            &bits("00000001000000100000001100000100")
        );
        assert_eq!(
            Cidr::parse("3.5.7.9/32").bits(),
            &bits("00000011000001010000011100001001")
        );
    }

    #[test]
    fn tree_insert() {
        let cidrs = vec![
            Cidr::parse("255.0.0.0/8"),
            Cidr::parse("255.100.0.0/16"),
            Cidr::parse("254.100.0.0/16"),
            Cidr::parse("13.14.15.16/32"),
        ];
        let mut tree = Tree::new();

        cidrs.iter().for_each(|c| tree.insert(c));

        assert_eq!(tree.cidrs(), 3);
        assert_eq!(tree.nodes(), 1 + 8 + 9 + 32);
        assert_eq!(
            tree.coverage(),
            1.0 / 256.0 + 1.0 / 65536.0 + 1.0 / 4294967296.0
        );
    }

    #[test]
    fn cidr_parse() {
        assert_eq!(Cidr::parse("1.2.3.4/8").to_pretty_string(), "1.0.0.0/8");
        assert_eq!(Cidr::parse("42.43.44.45/24").to_pretty_string(), "42.43.44.0/24");
        assert_eq!(
            Cidr::parse("255.255.255.255/32").to_pretty_string(),
            "255.255.255.255/32"
        );
    }
}
