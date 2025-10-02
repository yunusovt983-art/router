# C4 Component Diagram - Подробное объяснение Task 2

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_COMPONENT.puml`

Компонентная диаграмма детализирует внутреннюю структуру UGC подграфа, показывая взаимодействие между слоями архитектуры и их реализацию в коде.

## Архитектурные слои и их реализация

### 1. GraphQL Layer

#### Query Resolvers
```plantuml
Component(query_resolvers, "Query Resolvers", "async-graphql", "GraphQL Query резолверы..")
```

**Архитектурная роль**: Обработка GraphQL запросов на чтениных

**Реализация в коде**:
```rust
// crates/ugc-subgraph/src/resolvers/query.rs
use async_graphql::{Object, Context, Result, ID};
use shared::types::{OfferId, ReviewId, UserId};

ault)]
pub struct Query;

#[Object]
impl Query {
    /// Получение отзыва по 
    async fn review(
        &self,
        ctx: &_>,
        id: ID,
    ) -> Resultw>> {
        let service = ctx.data::<()?;
        let review_id = ReviewId::from_str(&id)?;
        
         запроса
        let span = tracing::info_span!("q;
        let _enter = span.enter();
        
        match seawait {
            Ok(review) => {
         ;
     w))
    
            Err(UgcError::NotFound { .. }) => {
                tracing::info!("Review not ound");
                Ok(No
            }
            Err(e) => {
                tracing::erro);
                Err(e.into())
            }
        }
    }
    
    /// Получение отзывов по объявл
    async fn reviews(
        elf,
        ctx: &Context<'_>,
        offer_id: ID,
        first: Option<i32>,
        after: Optring>,
        filter: Option<>,
    ) -> Result<ReviewConion> {
        lece>()?;
        let offer_id = OfferId::from?;
        
        // Валидация пагинации
        let args = ConnectionArgs {
            first: first.unwrap_or(10).min(100), // Максимум 100 за раз
            after,
            ..Default::default()
        };
        ar
        
        // Применение фильтров
     );
    
        let span = tracing::info_span!(
            "query_reviews", 
            offer_id = %offer_id,
            fit,
            has_filter = !
        );
        let _enter = span.ente();
        
        service.get_reviews_connection(offer_id, args
    }
    
    /ления
    ing(
        &self,
        ctx: &Context<'_>,
        offer_id: ID,
    ) -> Resul
        let service = ctx.ce>()?;
        let offer_id = OfferId::from
        
        let span = tracing::info_span!("query_offer_ratid);
        let _enter = span.enter();
        
        service.get_offer_rating(offer_id).await
    }
}
```

#### Mutation Resolvers
```plantuml
Component(mutation_resolvers, "Mut
```

**Архитектурная роль**: Ох

:
```rust
// crates/ugc-subgraph/src/resolvers/mu
use async_graphql::{Object, Context, Result, ID, Guard;
use shared::auth::{RequireAuth, Requir

#[derive(Default)]
pub struct Mutation;

#[Obj
i
   ии)
eAuth")]
    async fn create_rev
        &se,
        ctx: &Context<'_>,
   
ew> {
        let service = ctx.data::<ReviewService>()?;

        
       !(
            "mutation_create_review",
            user_id = %user_context.user_id,
            offer_id = %input.offer_id,
            rating = input.rating
 );
        let _enter =
  
        /
        input.v)?;
        
        let review = service.create_r.await?;
        
        // Метрики
        shared.inc();
        shared::metrics::R
            .with_label_values(&[
            .inc();
        
        tracing::info!(review_id = %review.id, "Review);
        
    }
    
    /// дератор)
    #[graphql(guard = "RequireAut
    async fn update_review(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateReviewInput,
    ) -> Result<Review> {
        let serviice>()?;
        ;
        let review_id = ReviewId::from_str(&id)?;
        
    
            "mutation_update_review",
            review_id = %review_id,
            user_id = %user_context.user_id
        );
        let _e
        
        // Пров
        service.check_edit_permis?;
        
        let updated_review = service.update_review(await?;
        
        shared::metrics::REVIEWS_UPDATED_TOTAL.inc();
        
        
        Ok(updated_review)
    }
    
    /)
    ]
    async fn moderate_review(
        &self,
        ctx: &Context<'_>,
        review_id: ID,
        actionn,
        reason: Option<Str>,
    ) -> Result {
        let service = ctx.data:?;
        let user_contex
        let review_id = ReviewId::from_str(&review_
        
        let span = tracing::info_span!(
        ",
            review_id = %review_id,
            moded,
     n
    ;
        let _enter = span.enter();
        
        let moderated_review = service.modeview(
            review_id,
            action,
            reason,
            uset,
        ).await?;
        
        shared::metrics::OTAL
            .with_label_values(&[&format!("{:?}", actio])
            .inc();
        
        );
        Ok(moderated_review)
    }
}
```

#### Federation Types
```plantuml
Component(fede")
```

**Архитектурная роль**: Определенграфами

**Реализация федеративных расширений**:
```rust
// crates/ugc-subgraph/src/resolvers/federation.rs
use asynt};

/// Расширение типа User из Users п
#[derive(SimpleObject)]
#[graphqltends)]
pub strur {
    #[graphql(external)]
    pub id: ID,
}

#[Object]
impl User {
    /User
 ntity)]
{
        User { id }
    }
    
    /// Получение всех отзвателя
    async fn reviews(
 
   ,

        after: Option,
    ) -> Re
        let service = ctx.data::<ReviewService>()?;
   
    
        let args = ConnectionArgs {

            after,
       fault()
        };
        

    }
    
    /// Статистика вателя
    async fn review{
        let service = ctx.data::<ReviewS
        let use?;
        
        service.get_wait
    }
}

/// Расширение типа Offer из Offer
#[derive(SimpleObject)]
#]
fer {
    #[graphql(ex
    pub id: I,
}

#[Object]
impl Offer {
    /// Entity resolver для Offer
    #[grapty)]
    affer {
    { id }
    }
    
    /// Получение отзывов объявления
    async fn revie
        &self,
        ctt<'_>,
     
    ng>,
        filter: Option<ReviewFilter>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        let args = ConnectionArgs {
     
 
:default()
        };
        
        let filter fault();
        service.g
    }
    
 тинга

        l?;
        letd)?;
        
        match service{
            Ok)),
            Err(UgcError::
            Err(e) => Err(e
        }
    }
}

/// Осно
#[derive(SimpleObject)]
#[graphql(extends)]
pub sw {
    ]
    pub id: ID,
    pub offer_id: ID,
    pub user_id: ID,
    pub rating: i32,
    pub ,
    pub is_moderated: bool,
    p>,
 tc>,
}

#[Object]
impl Review {
    /// Entity resew
    #[graphql(entity)]
    async fn fi
 
d)?;
        
        servp(Some)
    }
    
    /// Федера
    async fn user(&self) -User {
        User { id: self.use
    }
    
    /// Федеративная ссылка на объяение
    async fn offer(&self) -> Offer {
        Offer { id: self.offer_id.clone() }
    }
}
```

### 2. Middleware Layer

#### Aut
```plantuml
Component(auth_guard, "Auth Guard", "async-graphql Guard", "Пров")
```

**Архитектурная роль**: Защита GraphQL опацию

**Реализация Guard'ов**:
```rust
// crate.rs
use async_graphql::{Guard, Context, Result, Error};
use jthm};


pub struct RequireAuth;

#[async_trai_trait]
impl Guard for RequireAuth {
    async fn check(&se
        // Извлечение JWT токенв
        let tox
            .data_opt::<St)
            .and_then(|headers|ders))
            .ok_or_else(|
        
        // Валидация JWT токена
        
        let claims = jwt_service.validate_token(&tawait
            .map_err(|e| Error::new(format!("Invalid )))?;
      
    еля
        let user_context = UserContext 
            user_id: cr_id,
            roles: claims.roles,
            pesions,
            session_id: cl,
        };
        
        // Сохранение контекста в GraphQL Context
        ctx.insert(user_context);
        
        tracing::debug!(
     d,
 "
   );
    
        Ok(())
    }
}

/// Guard для проверки ролей
pub

#[async_trait::async_trait]
Role {
    async fn check(&se<()> {
       фикацию
        RequireAuth.check(ctx).await?;
        
        // Получаем контекст пользователя
xt>()?;
        
        // Проверяем на
 {
            tracing::warn!(
                user_id = %u
                required_role = ?self.0,
                user_roles = ?user_co.roles,
                "Access denied: insufficient permissions"
            );
        
 (

                self.0
            )));

        
        tracing::debug!(
            user_id = %user_context.user_id,
            role = ?self.0,
            "Role check passed"
        );
        
        Ok(())
    }
}

/// Иon
fg> {
") {
        Some(auth_header[7..].to_string())
    } else {
    None
    }
}
```

#### Rate Limiter
```plantl
Component(rate_limiter, "Rate Limiter", "Custom M")
```

**Архитектурная роль**: Защита от злоупотреблений через ограничениев

**Реа:
```rust
it.rs
use axum::{
    extract::{Request, State},
Map},
    middleware::Next,
    response::Response,
};
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub requests_per_h,
    pub burst_limit: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
        : 60,
            requests_per_hour: 1000,
            burst_0,
        }
    }
}

p(
ent>,
    headers: HeaderMap,
    request: Request,
ext,
) -> Result<Response, Statu> {
    // Извлечение идентификатора пользоля
    let user_id = extract_user_id_from_headers(&headers)
        .unwrap_or_else(|| "anonymous".to_string());
    
    // Пов
    let mut conn = redist
        .map_err(|_| StatusCode::INTERNAL_S?;
    
    let c
    let )
        .duration_since(UNIX
        .unwrap()
     
    
   ита

    let minute_co
        .maRROR)?;
    
   
it
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    
    if _minute {
        tracing::warn!(
            user_id = %user_id,
            count =nt,
            limit = confnute,
            "Rate limit exceeded (pminute)"
        );
);
    }
    
    // Проверка часового лимита
 600);
, 1).await
        .map_err
    
    if hour_count == 1 {
        let _: () = conn.e
 

    
    if hour_count > config.requests_per_hour {
        tracin(
            user_id = %user_id,
            count ount,
         ,
     
        );
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    // Метрики
    shared::metrics::RAT
    shared::metrics::RATE_SAGE
        .with_label_valuete"])
        .set(minute_count as f64);
    
    tracug!(
        user_id = %user_id,
        minute_count = minute_count,
        hour_count = hour_count,
        "Rate limit check passed"
    );
    
    Ok(next.run(req
}

fn extrac {
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
     | {
     {
                // Здесь можно декодиd
              токена
                So
            } else {
                None
            }
        })
}
```

### 3. Service Layer

#### Review Service
```plantuml
Componen")
```

**Архитектурная роль**: Центральнами

**Реализация сервиса**:
```rust
// crates/ugc-subgraph/src/serce.rs
use shared::types::{UserId, OfferIdiewId};
use shared::errors::sult;

pub struct ReviewService {
    repository: Arc<dyn ReviewRepositoryTrait>,
    rating_service: Arc<Ratiice>,
    validation_service: Arc<Validat
    cache_service: Avice>,
}

impl ReviewService {
    pub f(
        ,
        rating_service: Arc<RatingSe>,
        validation_service: Arc<Validae>,
        cache_service: Arc<C
    ) -> Self {
        Self {
            repository,
            rating_s,
        
            cache_service,
        }
    }
    
    /// Создание нового отзыва
    pub async fn cre
        &self,
        input: Cput,
     ext,
    w> {
        let span = tracing::inf
            "s
            user_i
            offer_id = %in
        );
        let _enter = span.enter();
        
        // 1. Валидация входных дх
        ?;
        
        // 2. Проверка на дублироотзыва
        if self.repository.hasfer(
           
        ,
        ).await? {
            return Err(UgcError::DuplicateResource {
                message: "User has already reviewed this offer".to_string(),
                conflicting_field: Some("us)),
            });
        
        
        // 3. Санитизация текста отзыва
        let sanitized_input = CreateReviewInput {
            text: self.validation_service.s,
            ..input
        };
        
        
        let review = self.repositoryw(
            sanitized_input,
        
        ).await?;
        
 га
rvice);
        let offer_id = review.offe
        tokio::spawn(asyn
            if let Err(e) = rating_service.update_offer_rating
                tra:error!(
                    offer__id,
                    error = %e,
      "
 );
 }
        });
        
        // 6. Инвалидацкешей
        self.invalidat?;
        
        // 7. Метрики и логирование
        shared::metrics::REVIEWS_CREATED_TOTnc();
        shared::metrics::REVIEWS_BY_RATING
            .with_label_values(&[&review.rating.to_string()])
            .inc();
        
        tracing::info!(
            review_id = %review.id,
            rating = review.rating,
            "Review crssfully"
        );
        
        Ok(review)
    }
    
    /// Получение отзывов с пагинацией
    pub async fn get_reviews_connection(
     
    
        args: ConnectionArgs,
 wFilter,
    
        let span = tracing::info_sp!(
",
            offer_i_id,
           first
        );
   
 
        // Проверка кеша для популярных запросов
format!(
            "reviews:{}:{}:{}:{}",
       _id,
            args.first,
            args.after.as_der""),
            filter.)
        );
        
        if let Some(cached_connection) = svice

            .await? 
        {
            tracing::debug!("
            return Ok(cached_connection;
        }
        
 зитория
ry
            .get_revfilter)
            .await?;
        
        // Построение connection
        let connection = self.build_review_)?;
        
        // Кеширование результата на 5 минут
        self.ca
            .s00)
            .await?;
        
        tracing::debug!(
            reviews_count = con(),
            has_next_page = page,
         t"
      );
        
        Ok(connection)
    }
    
    /// Проверка прав на редактирва
    pub async fn check_edit_permiss
        &self,
        review_id: ReviewId,
        user_context: &UserContext,
    ) ->)> {
        let review = self.repositoryit?
            .ok_or_else(|| UgcError::not_found("Review", reviewg()))?;
        
        // Автор может редактировать свой отзыв
        if review.user_id == user_context.user_id {
        ());
        }
        
        зыв
        if user_context.has_ro {
            return Ok(());
        }
        
        Err(UgcError::Forbidden {
        g(),
        })
    }
    
    /// Инвалидаци кешей
    as(
    f,
        offer_id: OfferId,
        user_irId,
    ) -> UgcResult<()> {
        // Инвалидация кеша отзывов
        let offer_patter
        self.cache_service.delete_patteit?;
        
        // Инвалидация кеша отзывов пользователя
        let user_pattern = format!("user_reviews:{}:*", user_id);
        self.cache_service.delete_pattern(&user_pattern).await?;
        
        /ния
        it?;
        
        Ok(())
    }
    
    /// Построение ReviewConnection из данных
    fn build_rection(
        &self,
        w>,
        args: ConnectionArgs,
        has_next_page: bool,
    ) -> UgcResult<ReviewConnection> {
        let edges: Vec<ReviewEdge> = reviews
            .into_iter()
            .ma
         
        Edge {
              
     or,
        }
            })
            .c;
        
        let page_info = PageI
            has_next_page,
            has_previous_page: 
            start_cursor: edges.()),
            end_cursor: edges.last().ma)),
        };
        
        Okction {
        
            page_info,
            total_count: None, // Можно добавить отдельный запрос для подсчета
        })
    }
}
```

### 4. Repository Layer

#### Revry
```plantuml
Component(review_repository, "Review Reposiов...")
```

**Архитектурная роль**: АбстрSQL

**Реализация репозитор:
```rust
// crates/ory.rs
use sqlx
use shared::types::{UserId, OfferwId};

#[async_trait::async_trait]
pub traid + Sync {
    async fn create_review(
     
 ut,
   erId,
w>;
    
    async f;
    
   (
f,
        offer_id: OfferId,
       ,
        filter: &ReviewFilter,
    ) -> UgcResult<(Vec<Re;
    
    async fn has_user_reviewed_offer(
 ,
rId,
        offer_id: Ofd,
    ) -> UgcResult<bool>;
    
    async fn update_review(
        &self,
        review: Review,
    ) -> UgcResult<Review>;
}

pub struct ReviewRepository {
    poolPgPool,
}

impl ReviewRepository {
    pub f {
        Self { pool }
    }
}

#[asyit]
implry {
    async fn create_review(
        &self,
        input: CreateReviewInput,
        user_id: UserId,
    ) ->ew> {
        let review = sqlx::_as!(
            Review,
           r#"
            INSERT INTO revis (
                offer_id, 
                user_id, 
        g, 
              
     t,
 t
   
))
            RETURNING 

                offer_id,
           
                rating,
   ext,

                moderated_by,
       
                moderation_reason,
              ,
                flag_reason,
                created_at,
                updated_at,
                deleted_at
            "#,
            input.offer_id.0,
 ,

            input.text
        )
 

        .map_e| {
            match &e {
                sqlx::Error::Database(rr) => {
                    if let Some(contraint() {
              nstraint {
                            "uniqu {
                              {
                                    message: ),
                                    confli,
                                }
        }
                            "reviews_rating_che {
                                UgcError::ValidatiError {
                                    message: "Rating must be between 1 tring(),
                                    field: Some("rating".to_string()),
                                }
            }
        :from(e),
                        }
                    } else {
                        UgcError::from(e)
                    }
            }
        
            }
        })?;
        
        t
        w.id,
            user_id = %user_id,
            offer_id = %input.offer_id,
         se"
        );
        
        Ok(review)
    }
    
    async fn ion(
        &self,
        offer_id: OfferId,
        args:
        filter: &ReviewFilter,
    ) -> UgcResult<(Vec<Review>, bool)> {
        let lpage
        l;
        
        // Построение динамического Wя
        let mut where_conditions = vec!["r.offer_id = $1", ""];
        let mut param_count = 1;
        
        
        if filter.only_moderated.unwratrue) {
            where_conditions.push("r.is
        }
        
        
            param_count += 1;
            where_conditions.push(&format!("r.rating >= $nt));
        }
        
        if let Some(max_rating) = filter.max_rating {
        
            where_conditions.push(&format!("r.rating <= ${}", param_count));
        }
        
     );
 
   Y

            "rating" => "r.rating

            _ => 
        };
        
   (
   r#"
            SELECT 
       
                r.offer_id,
                r.user_id,
                r.rating,
                r.text,
                r.is_moderated,
                r._by,
_at,
                r.moderation_reason,
                r.iged,
                r.fn,
                r.cred_at,
                r.updated_
                r.deleted_at
            FROM revs r
            WHERE {}
            ORDER BY {}
            LIMIT ${}
            OFFSET ${}
            "#,
            where_clause,
            order_clause,
 1,
2
        );
        
        // Выполнение запроса с паметрами
        let mut query_builder = sqlx::query)
            .bind(offer_id.0);
        
    
            query_builder = query;
        }
        
     g {
    
        }
        
        query_builder = queryilder
            .bind(limit)
     t);
    
r
            .fetch_all(&self.pool)
            .await
            .map_err(UgcErom)?;
        
        // Проверка наличия следующей стницы
        let has_next
    
            reviews.pop(); // Удаляем лишний элемент
        }
        
 g!(

            reviews_coun
            has_next_page = has_next_page,
            "Reviews fetched fro
        );
        
     ))
 
  
    async fn has_user_reviewed_offer(
        &self,
        user_id: UserId,
        offer_id: OfferId,
    ) -> UgcResult<bool> {
        let exists = sqlx::query!(
         
     EXISTS(
          iews 
  $1 
   

            ) as exists
"#,
            user_id.0,
       _id.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(UgcError::from)?;
        
        Ok(exists.exists.unwrap_or(false)
    }
    
    /// Вычисление offset для пагинации
    async fn calculate_offset(&self, ar
     {
            // Декодирование курсора и зиции
            let decoded = base64::or)
                .map_err(|_| Uor {
                    mess
          tring()),
                })?;
            
    ded)
                .map_err(|_| UgcError::ValidationError {
                    message: "Invalid cursor encoding".to_ng(),
    
                })?;
            
    
                let review_id = ReviewId::from_st
                
     offset
                let position = sqlx::query!(
                    r#"
    ition
                    FROM reviews 
                    WHERE created_at > (
     
                        FROM reviews 
                        WHERE id = $1
      )
                    "#,
                    review_id.0
         )
                .fetch_one(&self.pool)
                .await
    from)?;
                
 ))
   
onError {
                    message: "Invalid string(),
       g()),
                })
            }
        } else {
            Ok(0)
        }
    }
}
```

## Взаимодействия между компонентами

### Поток создания отзыва
```rust
// Пых

// 1. GraphQL Mutation -> Mutatiolver
mutation CreateReview($input: CreateRevi{
  createReview(input: $input) {
    id
    
    text
    createdAt
  }
}

// 2.Guard
#[gr")]
async fn create_review(ctx, input) -> Result<Review>

// 3. Auth Guard -> JWT Validation
RequireAuth::che

// 4. Mutation Resolver -> Review Srvice
service.create_review(input, user_conte

// 5. Review Service -> Validation Service
validation_service.valida

// 6. Review Servitory
repository.create_review(sanitized_input, u

// 7greSQL
INSERT INTO reviews (offer_id, user_id, ra.)

// 8
tokio::spawn(ratin))

// vice

```

### Поток получения отзывов с кешированием
`rust
// Оптимизированный поток с кешированием

// 1. GraphQL Query -> Query Resolver
query GetReviews($offerId: ID!, $first: Int) {
  reviews(offerId: $offerId, first: $first) {
 } }
    pageInfo { hasNextPage } 2. Query Resolver -> Review Serviceice.get_reviews_connection(offer_id, args, filter).await/ 3. Review Service -й.циеентаеской имплем и фактичизайномм дтурны архитек междуую связьет прямбеспечивакоде, что оализацию в ную реконкретт имеет  компонены

Каждыйи системафамодгр п** с другимию интеграциюедеративнук
6. **Фботкой ошибонгом и обра мониторидакшену** срость к пно **Готовю
5.инациование и паг кеширези** черостводительн произптимизацию4. **О
ейавторизацией и ификацитентсти** с аусноему безопаксную систмплеых
3. **Кобазы данно пераций дот GraphQL оость** емассирую тролну2. **Птуры
ями архитекмежду слоенности** ние ответствткое разделе**Чет:

1. емонстрируеафа ддгр UGC пораммаагнентная дипоы

Ком
## Вывод``
ait
`on, 300).aw, &connectiche_keyh_ttl(&cae.set_witache_servicш)
cв кенение раce (сохhe Servi -> Cacrvice Review Sege)

// 7._pa, has_next argsews,evition(rew_connecld_reviction
build Conne-> Bui Service view

// 6. ReIT $2 LIMSCted_at DER BY crea $1 ORDEd = offer_i WHEREeviews* FROM rLECT 
SEostgreSQLository -> Peview Rep
// 5. Rt
awaiilter).gs, f aroffer_id,tion(ginas_with_pa.get_reviewositoryrepitory
os Review Rep ->e Miss. Cach4b }

// nnection;n cached_co retursome() {n.is_tioconnec cached_d data
ifReturn cacheHit -> a. Cache 
// 4ait
key).awcache_on>(&nectiviewCone.get::<Revicserша)
cache_рка керове(пce ervie S Cach>

/
serv

//
  }
}