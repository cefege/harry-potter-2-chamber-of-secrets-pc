#=============================================================================
# Unreal Render makefile for Linux.
#
# Revision history:
# 	* Created by Brandon Reinhart 
#=============================================================================

# Unreal directory. (Required by makefile-header.)
UNREAL_DIR	= ../..

# Include global definitions.
include $(UNREAL_DIR)/makefile-header

#-----------------------------------------------------------------------------
# Directories.
#-----------------------------------------------------------------------------

SRC_DIRS	= $(RENDER_SRC)
INC_DIRS	= $(RENDER_INC)
BUILD_DIR	= $(RENDER_BUILD)

#-----------------------------------------------------------------------------
# Compiler/linker options.
#-----------------------------------------------------------------------------

CXX		= $(STD_CXX)
CXXFLAGS	= $(STD_CXXFLAGS) -I$(RENDER_INC) \
			-I$(ENGINE_INC) -I$(CORE_INC) \
			-DGPackage=GPackageRender
LDFLAGS		= $(STD_LDFLAGS) -shared -Wl,-soname,$(RENDER_SONAME)
LIBS		= $(STD_LIBS) $(CORE) $(ENGINE)
AS		= $(STD_AS)
ASFLAGS		= $(STD_ASFLAGS)
LCFILE		= $(PSX2_LIB)/app.cmd

#-----------------------------------------------------------------------------
# Files.
#-----------------------------------------------------------------------------

OBJS =			\
	Render.o	\
	UnLight.o	\
	UnMeshRn.o	\
	UnRandom.o	\
	UnRender.o	\
	UnSoftLn.o	\
	UnSpan.o	\
	UnSprite.o

OUT = $(RENDER)

#-----------------------------------------------------------------------------
# Rules.
#-----------------------------------------------------------------------------

# Use general rules.
include $(UNREAL_DIR)/makefile-general
	
#-----------------------------------------------------------------------------
# The End.
#-----------------------------------------------------------------------------
