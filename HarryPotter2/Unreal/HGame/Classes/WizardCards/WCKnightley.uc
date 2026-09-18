//===============================================================================
// #74	Montague Knightley
//===============================================================================

class  WCKnightley extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardKnightleyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Knightleysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardKnightleyBigTexture FILE=TEXTURES\menu\Folio\Cards\Knightleybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardMontagueBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\74_Montague_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardMontagueBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\74_Montague_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardMontagueBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\74_Montague_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Montague Knightley";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=74
    skin(0)=Texture'HProps.skins.WizardCardKnightleyTex0'
	textureBig=Texture'WizCardKnightleyBigTexture'
	strDescriptionId="WizCard_0068"
	bIsLayered=true;
	textureLayers(0)=Texture'WizCardMontagueBigTextureLayer0';
	textureLayers(1)=Texture'WizCardMontagueBigTextureLayer1';
	textureLayers(2)=Texture'WizCardMontagueBigTextureLayer2';

}
