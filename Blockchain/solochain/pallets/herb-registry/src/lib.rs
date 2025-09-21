
// Custom Substrate Pallet for Ayurvedic Herb Traceability
// File: pallets/herb-registry/src/lib.rs

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::{DispatchResult, DispatchResultWithPostInfo},
        pallet_prelude::*,
        traits::{Get, Randomness},
    };
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;
    use codec::{Encode, Decode};

    // Pallet configuration trait
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Maximum length for herb species name
        #[pallet::constant]
        type MaxSpeciesNameLength: Get<u32>;

        /// Maximum number of quality metrics per collection
        #[pallet::constant] 
        type MaxQualityMetrics: Get<u32>;

        /// Randomness source for batch ID generation
        type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // Data structures for herb traceability
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct BatchInfo<AccountId, BlockNumber> {
        pub batch_id: [u8; 32],
        pub collector: AccountId,
        pub species: BoundedVec<u8, ConstU32<64>>,
        pub coordinates: GeoCoordinates,
        pub collection_time: BlockNumber,
        pub status: BatchStatus,
        pub quality_metrics: BoundedVec<QualityMetric, ConstU32<10>>,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct GeoCoordinates {
        pub latitude: i32,  // Scaled by 1e6 for precision
        pub longitude: i32, // Scaled by 1e6 for precision
        pub altitude: Option<u16>, // Meters above sea level
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct QualityMetric {
        pub metric_type: BoundedVec<u8, ConstU32<32>>,
        pub value: BoundedVec<u8, ConstU32<64>>,
        pub unit: BoundedVec<u8, ConstU32<16>>,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub enum BatchStatus {
        Collected,
        InTransit,
        Processing,
        QualityTested,
        Packaged,
        Delivered,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct CollectorProfile<AccountId> {
        pub collector_id: AccountId,
        pub name: BoundedVec<u8, ConstU32<64>>,
        pub certification_level: u8,
        pub approved_species: BoundedVec<BoundedVec<u8, ConstU32<64>>, ConstU32<50>>,
        pub total_collections: u32,
        pub reputation_score: u16,
    }

    // Storage items
    #[pallet::storage]
    #[pallet::getter(fn batches)]
    pub type Batches<T: Config> = StorageMap<
        _,
        
        Blake2_128Concat,
        [u8; 32], // batch_id
        BatchInfo<T::AccountId, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn collectors)]
    pub type Collectors<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        CollectorProfile<T::AccountId>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn herb_species)]
    pub type HerbSpecies<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<64>>, // species name
        BoundedVec<u8, ConstU32<256>>, // species metadata (JSON)
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn batch_count)]
    pub type BatchCount<T> = StorageValue<_, u32, ValueQuery>;

    // Events emitted by this pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new herb collection has been registered
        CollectionRegistered {
            batch_id: [u8; 32],
            collector: T::AccountId,
            species: BoundedVec<u8, ConstU32<64>>,
            coordinates: GeoCoordinates,
        },

        /// Batch status has been updated
        BatchStatusUpdated {
            batch_id: [u8; 32],
            old_status: BatchStatus,
            new_status: BatchStatus,
        },

        /// A new collector has been registered
        CollectorRegistered {
            collector: T::AccountId,
            name: BoundedVec<u8, ConstU32<64>>,
        },

        /// Quality metrics have been added to a batch
        QualityMetricsAdded {
            batch_id: [u8; 32],
            metrics_count: u32,
        },
    }

    // Errors that can be returned by this pallet
    #[pallet::error]
    pub enum Error<T> {
        /// Batch does not exist
        BatchNotFound,
        /// Collector not registered
        CollectorNotRegistered,
        /// Invalid coordinates (out of range)
        InvalidCoordinates,
        /// Species name too long
        SpeciesNameTooLong,
        /// Too many quality metrics
        TooManyQualityMetrics,
        /// Collector already exists
        CollectorAlreadyExists,
        /// Batch already exists
        BatchAlreadyExists,
        /// Invalid batch status transition
        InvalidStatusTransition,
        /// Collector not authorized for this species
        CollectorNotAuthorized,
    }

    // Dispatchable functions (extrinsics)
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new collector in the system
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn register_collector(
            origin: OriginFor<T>,
            name: BoundedVec<u8, ConstU32<64>>,
            certification_level: u8,
            approved_species: BoundedVec<BoundedVec<u8, ConstU32<64>>, ConstU32<50>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure collector doesn't already exist
            ensure!(!Collectors::<T>::contains_key(&who), Error::<T>::CollectorAlreadyExists);

            let collector_profile = CollectorProfile {
                collector_id: who.clone(),
                name: name.clone(),
                certification_level,
                approved_species,
                total_collections: 0,
                reputation_score: 100, // Start with base reputation
            };

            Collectors::<T>::insert(&who, collector_profile);

            Self::deposit_event(Event::CollectorRegistered {
                collector: who,
                name,
            });

            Ok(())
        }

        /// Register a new herb collection event
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn register_collection(
            origin: OriginFor<T>,
            species: BoundedVec<u8, ConstU32<64>>,
            coordinates: GeoCoordinates,
            quality_metrics: BoundedVec<QualityMetric, ConstU32<10>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Ensure collector is registered
            let mut collector = Self::collectors(&who).ok_or(Error::<T>::CollectorNotRegistered)?;

            // Validate coordinates (basic range check)
            ensure!(
                coordinates.latitude >= -90_000_000 && coordinates.latitude <= 90_000_000 &&
                coordinates.longitude >= -180_000_000 && coordinates.longitude <= 180_000_000,
                Error::<T>::InvalidCoordinates
            );

            // Ensure collector is authorized for this species
            ensure!(
                collector.approved_species.contains(&species),
                Error::<T>::CollectorNotAuthorized
            );

            // Generate unique batch ID
            let current_block = <frame_system::Pallet<T>>::block_number();
            let (random_seed, _) = T::Randomness::random(&b"herb_batch"[..]);
            let batch_id = Self::generate_batch_id(&who, &species, random_seed.as_ref());

            // Ensure batch ID is unique
            ensure!(!Batches::<T>::contains_key(&batch_id), Error::<T>::BatchAlreadyExists);

            let batch_info = BatchInfo {
                batch_id,
                collector: who.clone(),
                species: species.clone(),
                coordinates: coordinates.clone(),
                collection_time: current_block,
                status: BatchStatus::Collected,
                quality_metrics,
            };

            // Store the batch
            Batches::<T>::insert(&batch_id, &batch_info);

            // Update collector stats
            collector.total_collections += 1;
            Collectors::<T>::insert(&who, &collector);

            // Update batch count
            let current_count = Self::batch_count();
            BatchCount::<T>::put(current_count + 1);

            Self::deposit_event(Event::CollectionRegistered {
                batch_id,
                collector: who,
                species,
                coordinates,
            });

            Ok(())
        }

        /// Update the status of a batch
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn update_batch_status(
            origin: OriginFor<T>,
            batch_id: [u8; 32],
            new_status: BatchStatus,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let mut batch = Self::batches(&batch_id).ok_or(Error::<T>::BatchNotFound)?;

            // Validate status transition (simplified)
            Self::validate_status_transition(&batch.status, &new_status)?;

            let old_status = batch.status.clone();
            batch.status = new_status.clone();

            Batches::<T>::insert(&batch_id, &batch);

            Self::deposit_event(Event::BatchStatusUpdated {
                batch_id,
                old_status,
                new_status,
            });

            Ok(())
        }
    }

    // Helper functions
    impl<T: Config> Pallet<T> {
        /// Generate a unique batch ID
        fn generate_batch_id(
            collector: &T::AccountId,
            species: &BoundedVec<u8, ConstU32<64>>,
            random_seed: &[u8],
        ) -> [u8; 32] {
            let mut input = Vec::new();
            input.extend_from_slice(&collector.encode());
            input.extend_from_slice(&species.encode());
            input.extend_from_slice(random_seed);

            sp_io::hashing::blake2_256(&input)
        }

        /// Validate batch status transitions
        fn validate_status_transition(
            current: &BatchStatus,
            new: &BatchStatus,
        ) -> Result<(), Error<T>> {
            use BatchStatus::*;

            let valid = match (current, new) {
                (Collected, InTransit | Processing) => true,
                (InTransit, Processing | Delivered) => true,
                (Processing, QualityTested) => true,
                (QualityTested, Packaged) => true,
                (Packaged, Delivered) => true,
                _ => false,
            };

            if valid {
                Ok(())
            } else {
                Err(Error::<T>::InvalidStatusTransition)
            }
        }
    }
}
