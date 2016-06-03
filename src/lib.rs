extern crate jservice;
extern crate rand;

use std::slice::{Iter, IterMut};

pub use jservice::{Clue, Category};

use rand::Rng;
use rand::distributions::{IndependentSample, Range};

pub struct Jeopardy {
    jeopardy_board: Board,
    double_jeopardy_board: Board,
    final_jeopardy_clue: Clue
}

impl Jeopardy {
    pub fn generate() -> Result<Jeopardy, String> {
        let mut used_ids = Vec::new();
        let jeopardy_board = generate_board(6, 1, &mut used_ids).unwrap();
        let double_jeopardy_board = generate_board(6, 2, &mut used_ids).unwrap();
        let final_jeopardy_clue = {
            let mut final_clue: Clue = jservice::get_random(Some(1)).unwrap()[0].clone();
            if jeopardy_board.borrow_flat_clues().iter()
                             .chain(double_jeopardy_board.borrow_flat_clues().iter())
                             .any(|ref clue| clue.id() == final_clue.id) {
                final_clue = jservice::get_random(Some(1)).unwrap()[0].clone();
            }

            final_clue
        };

        Ok(Jeopardy {
            jeopardy_board: jeopardy_board,
            double_jeopardy_board: double_jeopardy_board,
            final_jeopardy_clue: final_jeopardy_clue
        })
    }

    pub fn get_jeopardy_board(&self) -> Board {
        self.jeopardy_board.clone()
    }

    pub fn get_double_jeopardy_board(&self) -> Board {
        self.double_jeopardy_board.clone()
    }

    pub fn get_final_jeopardy_clue(&self) -> Clue {
        self.final_jeopardy_clue.clone()
    }
}

fn generate_board(num_categories: u32,
                  num_daily_doubles: u32,
                  used_ids: &mut Vec<u64>) -> Result<Board, String> {
    if num_daily_doubles > num_categories {
        return Err("Number of daily doubles cannot be greater than number of categories".to_owned())
    }

    const MAX_CATEGORY: u64 = 15000;

    let between = Range::new(7000, MAX_CATEGORY);
    let mut rng = rand::thread_rng();
    let mut categories: Vec<Category> = Vec::new();

    while (categories.len() as u32) < num_categories {
        let category = {
            let mut id = between.ind_sample(&mut rng);
            while used_ids.iter().any(|used_id| *used_id == id) {
                id = between.ind_sample(&mut rng);
            }

            used_ids.push(id);
            match jservice::get_category(id) {
                Ok(category) => { category }
                Err(e) => { return Err(e) }
            }
        };
        match shuffle_category(&category) {
            Some(shuffled_category) => { categories.push(shuffled_category) }
            None => { }
        }
    }

    let daily_doubles = {
        let mut daily_doubles: Vec<u64> = Vec::new();
        for category in rand::sample(&mut rng, categories.iter(), num_daily_doubles as usize) {
            let category = category.clone();
            daily_doubles.push(rand::sample(&mut rng, category.clues.unwrap().iter().map(|ref clue| clue.id), 1)[0]);
        }

        daily_doubles
    };

    if daily_doubles.len() != num_daily_doubles as usize {
        panic!("Number of required daily doubles does not match number of selected daily doubles.")
    }

    let board_categories: Vec<BoardCategory> = categories.iter().map(|ref category| {
        let category: Category = (*category).clone();
        BoardCategory::new(category.clone().clues.unwrap().iter().map(|ref clue| {
            BoardClue::new(match clue.value {
                               Some(200) => { BoardValue::TwoHundred }
                               Some(400) => { BoardValue::FourHundred }
                               Some(600) => { BoardValue::SixHundred }
                               Some(800) => { BoardValue::EightHundred }
                               Some(1000) => { BoardValue::OneThousand }
                               v => { panic!("Clue ID {} has invalid value {:?}", clue.id, v) }
                           }, daily_doubles.iter().any(|&id| id == clue.id), (*clue).clone())
        }).collect(), category)
    }).collect();

    let board = Board::new(board_categories);

    Ok(board)
}

fn shuffle_category(category: &Category) -> Option<Category> {
    let mut rng = rand::thread_rng();
    let mut category = category.clone();

    let clues: Vec<Clue> = [200, 400, 600, 800, 1000].iter().filter_map(|value| {
        get_clue_sample(&category, Some(*value), &mut rng)
    }).collect();

    // if there are not 5 clues, then we have problems
    if clues.len() != 5 {
        return None
    }

    category.clues = Some(clues);

    Some(category)
}

fn get_clue_sample(category: &Category, value: Option<i32>, mut rng: &mut Rng) -> Option<Clue> {
    let clues = category.clone().clues.unwrap();
    match rand::sample(&mut rng,
                       clues.iter().filter(|ref clue| clue.value == value),
                       1).first().map(|ref clue| clue.clone())
    {
        Some(clue) => { Some((*clue).clone()) }
        None => {
            if value.is_none() {
                None
            } else {
                let mut clue = get_clue_sample(&category, None, &mut rng).expect(&format!("Category {} has no valid clue for value {:?}", category.id, value));
                clue.value = value;

                Some(clue)
            }
        }
    }
}

#[derive(Clone)]
pub struct Board {
    pub categories: Vec<BoardCategory>
}

impl Board {
    pub fn new(categories: Vec<BoardCategory>) -> Board {
        Board {
            categories: categories
        }
    }

    pub fn active_clues(&self) -> usize {
        self.borrow_flat_clues().iter().fold(0, |acc, &clue| acc + if clue.active { 1 } else { 0 })
    }

    pub fn get_category_by_id(&self, id: u64) -> Option<&BoardCategory> {
        self.iter_categories().find(|cat| cat.id() == id)
    }

    pub fn iter_categories(&self) -> Iter<BoardCategory> {
        self.categories.iter()
    }

    pub fn iter_categories_mut(&mut self) -> IterMut<BoardCategory> {
        self.categories.iter_mut()
    }

    pub fn borrow_flat_active_clues_mut(&mut self) -> Vec<&mut BoardClue> {
        self.categories.iter_mut().flat_map(
            |category| category.clues.iter_mut()
        ).filter(|clue| clue.active).collect::<Vec<&mut BoardClue>>()
    }

    pub fn borrow_flat_active_clues(&self) -> Vec<&BoardClue> {
        self.categories.iter().flat_map(
            |ref category| category.clues.iter()
        ).filter(|clue| clue.active).collect::<Vec<&BoardClue>>()
    }

    pub fn borrow_flat_clues_mut(&mut self) -> Vec<&mut BoardClue> {
        self.categories.iter_mut().flat_map(
            |category| category.clues.iter_mut()
        ).collect::<Vec<&mut BoardClue>>()
    }

    pub fn borrow_flat_clues(&self) -> Vec<&BoardClue> {
        self.categories.iter().flat_map(
            |ref category| category.clues.iter()
        ).collect::<Vec<&BoardClue>>()
    }
}

#[derive(Clone)]
pub struct BoardCategory {
    clues: Vec<BoardClue>,
    inner: Category
}

impl BoardCategory {
    pub fn new(clues: Vec<BoardClue>, inner: Category) -> BoardCategory {
        BoardCategory {
            clues: clues,
            inner: inner
        }
    }

    pub fn iter_clues(&self) -> Iter<BoardClue> {
        self.clues.iter()
    }

    pub fn iter_clues_mut(&mut self) -> IterMut<BoardClue> {
        self.clues.iter_mut()
    }

    pub fn get(&self, value: BoardValue) -> Option<&BoardClue> {
        self.clues.iter().find(|&clue| clue.board_value == value)
    }

    pub fn get_mut(&mut self, value: BoardValue) -> Option<&mut BoardClue> {
        self.clues.iter_mut().find(|clue| clue.board_value == value)
    }

    pub fn inner(&self) -> &Category {
        &self.inner
    }

    pub fn id(&self) -> u64 {
        self.inner.id
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum BoardValue {
    TwoHundred = 200,
    FourHundred = 400,
    SixHundred = 600,
    EightHundred = 800,
    OneThousand = 1000
}

#[derive(Clone)]
pub struct BoardClue {
    pub board_value: BoardValue,
    pub daily_double: bool,
    pub active: bool,
    pub inner: Clue
}

impl BoardClue {
    pub fn new(board_value: BoardValue, daily_double: bool, inner: Clue) -> BoardClue {
        BoardClue {
            board_value: board_value,
            daily_double: daily_double,
            active: true,
            inner: inner
        }
    }

    pub fn value(&self, multiplier: i32) -> i32 {
        (self.board_value as i32) * multiplier
    }

    pub fn id(&self) -> u64 {
        self.inner.id
    }
}
