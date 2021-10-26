use std::cell::Ref;

use lux_ai::{Action, Agent, Annotate, Cell, City, CityTile, Commands, Direction::*, Environment,
             LuxAiError, LuxAiResult, Position, Resource, ResourceType::*, Unit, UnitType::*};

struct Engine {
    environment:        Environment,
    agent:              Agent,
    eligible_resources: Vec<Cell>,
}

impl Engine {
    fn new() -> LuxAiResult<Self> {
        let mut environment = Environment::new();
        let agent = Agent::new(&mut environment)?;
        Ok(Engine {
            environment,
            agent,
            eligible_resources: Vec::new(),
        })
    }

    fn is_day(&self) -> bool { self.agent.turn % 40 < 30 }

    fn is_night(&self) -> bool { !self.is_day() }

    fn turns_until_night(&self) -> Option<i32> {
        if self.is_night() {
            return None;
        }

        Some(30 - (self.agent.turn % 40))
    }

    fn turn(&mut self) -> LuxAiResult<()> {
        self.agent.update_turn(&mut self.environment)?;
        self.update_eligible_resources();

        let player = self.agent.player().clone();

        for unit in player.units.iter() {
            match unit.unit_type {
                Worker if unit.can_act() =>
                    if let Some(action) = self.turn_worker(unit)? {
                        self.environment.write_action(action);
                    },
                Cart if unit.can_act() =>
                    if let Some(action) = self.turn_cart(unit)? {
                        self.environment.write_action(action);
                    },
                _ => {},
            }
        }

        for (_, city) in player.cities.into_iter() {
            for citytile in city.citytiles.iter() {
                let citytile = citytile.borrow();
                if citytile.can_act() {
                    if let Some(action) = self.turn_citytile(citytile)? {
                        self.environment.write_action(action);
                    }
                }
            }
        }

        self.environment.flush_actions()?;
        self.environment
            .write_raw_action(Commands::FINISH.to_string())?;
        self.environment.flush()?;

        Ok(())
    }

    fn closest_city_to(&self, pos: &Position) -> Option<Ref<CityTile>> {
        // Else if no cargo space left
        let mut closest_distance = f32::MAX;
        let mut closest_city_tile: Option<Ref<CityTile>> = None;

        // Find nearest city tile
        for city in self.agent.player().cities.values() {
            for city_tile in city.citytiles.iter() {
                let city_tile = city_tile.borrow();
                let distance = city_tile.pos.distance_to(pos);

                if distance < closest_distance {
                    closest_distance = distance;
                    closest_city_tile = Some(city_tile);
                }
            }
        }

        closest_city_tile
    }

    fn closest_eligible_resource_to(&self, pos: &Position) -> Option<&Cell> {
        let mut closest_distance = f32::MAX;
        let mut closest_resource_cell: Option<&Cell> = None;

        for resource_cell in self.eligible_resources.iter() {
            let distance = resource_cell.pos.distance_to(pos);
            if distance < closest_distance {
                closest_distance = distance;
                closest_resource_cell = Some(resource_cell);
            }
        }

        closest_resource_cell
    }

    fn position_in_bounds(&self, pos: &Position) -> bool {
        pos.x >= 0 &&
            pos.y >= 0 &&
            pos.x < self.agent.game_map.width &&
            pos.y < self.agent.game_map.height
    }

    fn empty_cell_adjacent_to(&self, pos: &Position) -> Option<&Cell> {
        let directions = vec![North, South, East, West];
        for direction in directions {
            let pos = pos.translate(direction, 1);
            if !self.position_in_bounds(&pos) {
                continue;
            }
            let cell = &self.agent.game_map[pos];
            if cell.citytile.is_none() && !cell.has_resource() {
                return Some(cell);
            }
        }
        None
    }

    fn turn_cart(&mut self, cart: &Unit) -> LuxAiResult<Option<Action>> { return Ok(None) }

    fn turn_citytile(&mut self, citytile: Ref<CityTile>) -> LuxAiResult<Option<Action>> {
        let player = self.agent.player();
        if player.city_tile_count > player.units.len() as u32 {
            return Ok(Some(citytile.build_worker()));
        }

        Ok(None)
    }

    fn turn_worker(&mut self, worker: &Unit) -> LuxAiResult<Option<Action>> {
        if worker.cargo_space_used() >= City::city_build_cost() {
            if worker.can_build(&self.agent.game_map) {
                return Ok(Some(worker.build_city()));
            }

            if let Some(city) = self.closest_city_to(&worker.pos) {
                if let Some(empty_cell) = self.empty_cell_adjacent_to(&city.pos) {
                    return Ok(Some(worker.move_(worker.pos.direction_to(&empty_cell.pos))));
                }
            }
        }

        if worker.get_cargo_space_left() > 0 {
            if let Some(cell) = self.closest_eligible_resource_to(&worker.pos) {
                return Ok(Some(worker.move_(worker.pos.direction_to(&cell.pos))));
            }
        }

        if worker.get_cargo_space_left() == 0 {
            if let Some(city) = self.closest_city_to(&worker.pos) {
                return Ok(Some(worker.move_(worker.pos.direction_to(&city.pos))));
            }
        }

        Ok(None)
    }

    fn update_eligible_resources(&mut self) {
        self.eligible_resources = Vec::new();
        for y in 0..self.agent.game_map.height() {
            for x in 0..self.agent.game_map.width() {
                let position = Position::new(x, y);
                let cell = &self.agent.game_map[position];
                if let Some(resource) = &cell.resource {
                    if self.is_resource_eligible(resource) {
                        self.eligible_resources.push(cell.clone());
                    }
                }
            }
        }
    }

    fn is_resource_eligible(&self, resource: &Resource) -> bool {
        if !self.agent.player().is_researched(resource.resource_type) {
            return false;
        }

        match resource.resource_type {
            Wood if resource.amount > 400 => true,
            Coal => true,
            Uranium => true,
            _ => false,
        }
    }
}

fn main() -> LuxAiResult<()> {
    let mut engine = Engine::new()?;
    loop {
        engine.turn()?;
    }
}
