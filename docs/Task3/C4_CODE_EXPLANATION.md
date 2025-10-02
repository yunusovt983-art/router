# C4 Code Diagram - Подробное объяснение Task 3

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_CODE.puml`

Диаграмма кода Task 3 показывает детальную структуру Integration Hub на уровне модулей, классов и их взаимосвязей в TypeScript/Node.js проекте.

## Структура проекта и реализация

### 1. API Module (src/api/)

#### Express Application
```plantuml
Component(express_app, "Express Application", "TypeScript", "const app = express()...")
```

**Архитектурная роль**: Основное приложение Express с middleware

**Реализация**:
```typescript
// src/api/app.ts
import express, { Application, Request, Response, NextFunction } from 'express';
import cors from 'cors';
import helmet from 'helmet';
import compression from 'compression';
import rateLimit from 'express-rate-limit';
import { createServer } from 'http';
import { Server as SocketIOServer } from 'socket.io';

// Middleware
import { authMiddleware } from '../middleware/auth.middleware';
import { loggingMiddleware } from '../middleware/logging.middleware';
import { metricsMiddleware } from '../middleware/metrics.middleware';
import { errorHandler } from '../middleware/error-handler.middleware';

// Routes
import { integrationRoutes } from './routes/integration.routes';
import { monitoringRoutes } from './routes/monitoring.routes';
import { federationRoutes } from './routes/federation.routes';

export class IntegrationApp {
  private app: Application;
  private server: any;
  private io: SocketIOServer;
  
  constructor() {
    this.app = express();
    this.server = createServer(this.app);
    this.io = new SocketIOServer(this.server, {
      cors: {
        origin: process.env.ALLOWED_ORIGINS?.split(',') || ['http://localhost:3000'],
        methods: ['GET', 'POST'],
      },
    });
    
    this.setupMiddleware();
    this.setupRoutes();
    this.setupWebSocket();
    this.setupErrorHandling();
  }
  
  private setupMiddleware(): void {
    // Security middleware
    this.app.use(helmet({
      contentSecurityPolicy: {
        directives: {
          defaultSrc: ["'self'"],
          scriptSrc: ["'self'", "'unsafe-inline'"],
          styleSrc: ["'self'", "'unsafe-inline'"],
          imgSrc: ["'self'", "data:", "https:"],
        },
      },
    }));
    
    // CORS configuration
    this.app.use(cors({
      origin: (origin, callback) => {
        const allowedOrigins = process.env.ALLOWED_ORIGINS?.split(',') || [];
        if (!origin || allowedOrigins.includes(origin)) {
          callback(null, true);
        } else {
          callback(new Error('Not allowed by CORS'));
        }
      },
      credentials: true,
    }));
    
    // Rate limiting
    this.app.use(rateLimit({
      windowMs: 15 * 60 * 1000, // 15 minutes
      max: 1000, // limit each IP to 1000 requests per windowMs
      message: {
        error: 'Too many requests from this IP, please try again later',
        retryAfter: 15 * 60, // seconds
      },
      standardHeaders: true,
      legacyHeaders: false,
    }));
    
    // Body parsing
    this.app.use(express.json({ limit: '10mb' }));
    this.app.use(express.urlencoded({ extended: true, limit: '10mb' }));
    
    // Compression
    this.app.use(compression());
    
    // Custom middleware
    this.app.use(loggingMiddleware);
    this.app.use(metricsMiddleware);
    this.app.use(authMiddleware);
  }
  
  private setupRoutes(): void {
    // API routes
    this.app.use('/api/integration', integrationRoutes);
    this.app.use('/api/monitoring', monitoringRoutes);
    this.app.use('/api/federation', federationRoutes);
    
    // Health check
    this.app.get('/health', (req: Request, res: Response) => {
      res.json({
        status: 'healthy',
        timestamp: new Date().toISOString(),
        version: process.env.npm_package_version || '1.0.0',
        uptime: process.uptime(),
        memory: process.memoryUsage(),
      });
    });
    
    // Metrics endpoint
    this.app.get('/metrics', async (req: Request, res: Response) => {
      const { register } = await import('prom-client');
      res.set('Content-Type', register.contentType);
      res.end(await register.metrics());
    });
  }
  
  private setupWebSocket(): void {
    this.io.on('connection', (socket) => {
      console.log(`Client connected: ${socket.id}`);
      
      // Join monitoring room
      socket.join('monitoring');
      
      // Handle subscriptions
      socket.on('subscribe-metrics', (data) => {
        socket.join(`metrics-${data.subgraphs?.join('-') || 'all'}`);
      });
      
      socket.on('subscribe-alerts', () => {
        socket.join('alerts');
      });
      
      socket.on('disconnect', () => {
        console.log(`Client disconnected: ${socket.id}`);
      });
    });
  }
  
  private setupErrorHandling(): void {
    // 404 handler
    this.app.use('*', (req: Request, res: Response) => {
      res.status(404).json({
        error: 'Not Found',
        message: `Route ${req.originalUrl} not found`,
        timestamp: new Date().toISOString(),
      });
    });
    
    // Global error handler
    this.app.use(errorHandler);
  }
  
  public getSocketIO(): SocketIOServer {
    return this.io;
  }
  
  public listen(port: number): Promise<void> {
    return new Promise((resolve) => {
      this.server.listen(port, () => {
        console.log(`🚀 Integration Hub server running on port ${port}`);
        resolve();
      });
    });
  }
}
```

#### Integration Routes
```plantuml
Component(integration_routes, "Integration Routes", "Express Router", "router.get('/subgraphs', getSubgraphs)...")
```

**Реализация маршрутов**:
```typescript
// src/api/routes/integration.routes.ts
import { Router } from 'express';
import { IntegrationController } from '../controllers/integration.controller';
import { validateRequest } from '../middleware/validation.middleware';
import { requireRole } from '../middleware/auth.middleware';
import { 
  registerSubgraphSchema,
  updateSubgraphSchema,
  composeSupergraphSchema 
} from '../schemas/integration.schemas';

const router = Router();
const integrationController = new IntegrationController();

// Subgraph management routes
router.get('/subgraphs', 
  integrationController.getSubgraphs.bind(integrationController)
);

router.post('/subgraphs',
  requireRole(['developer', 'admin']),
  validateRequest(registerSubgraphSchema),
  integrationController.registerSubgraph.bind(integrationController)
);

router.get('/subgraphs/:id',
  integrationController.getSubgraph.bind(integrationController)
);

router.put('/subgraphs/:id',
  requireRole(['developer', 'admin']),
  validateRequest(updateSubgraphSchema),
  integrationController.updateSubgraph.bind(integrationController)
);

router.delete('/subgraphs/:id',
  requireRole(['admin']),
  integrationController.deleteSubgraph.bind(integrationController)
);

router.put('/subgraphs/:id/schema',
  requireRole(['developer', 'admin']),
  validateRequest(updateSubgraphSchema),
  integrationController.updateSubgraphSchema.bind(integrationController)
);

// Schema composition routes
router.post('/compose',
  requireRole(['developer', 'admin']),
  validateRequest(composeSupergraphSchema),
  integrationController.composeSupergraph.bind(integrationController)
);

router.get('/compose/validate',
  integrationController.validateComposition.bind(integrationController)
);

// Health check routes
router.get('/subgraphs/:id/health',
  integrationController.checkSubgraphHealth.bind(integrationController)
);

router.post('/subgraphs/health/bulk',
  integrationController.bulkHealthCheck.bind(integrationController)
);

export { router as integrationRoutes };
```